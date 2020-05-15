use serde_json::json;
use std::fmt::{Formatter, Display};
use once_cell::sync::OnceCell;
use crate::db;
use std::collections::HashMap;
use serde_derive::{Deserialize, Serialize};
use failure_derive::*;
use crate::entity::{Entity, EntityStore, EntityError};
use crate::authenticator::{Authenticator, AuthenticatorStore};

#[derive(Debug, Fail)]
pub enum IdentifierError {
    #[fail(display = "An identifier with of this type nad value already exists")]
    IdentifierExists(),
    #[fail(display = "Entity cannot not have empty {}", 0)]
    EmptyField(String),
    #[fail(display = "Identifier with uid does not exist")]
    DoesNotExist(),
    #[fail(display = "Cannot extract identifier value from an empty array or None value")]
    Empty()
}

impl From<String> for IdentifierError {
    fn from(e: String) -> Self {
        IdentifierError::EmptyField(e)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IdentifierRoot {
    pub identifier: Vec<Identifier>
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum IdentifierType {
    email,
    phone,
    username,
    public_key
}

impl Display for IdentifierType {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Identifier {
    pub uid: Option<String>,
    pub identifier_type: Option<IdentifierType>,
    pub value: Option<String>,
    #[serde(rename = "entity")]
    pub entities: Option<Vec<Entity>>,
    #[serde(rename = "authenticator")]
    pub authenticators: Option<Vec<Authenticator>>,
    #[serde(rename = "dgraph.type")]
    pub dtype: Option<Vec<String>>
}

impl Identifier {
    pub fn new() -> Identifier {
        Identifier {
            dtype: Some(vec!["Identifier".to_string()]),
            ..Default::default()
        }
    }

    pub fn uid(mut self, uid: String) -> Self {
        self.uid = Some(uid);
        self
    }

    pub fn identifier_type(mut self, identifier_type: IdentifierType) -> Self {
        self.identifier_type = Some(identifier_type);
        self
    }

    pub fn value(mut self, value: String) -> Self {
        self.value = Some(value);
        self
    }

    pub fn add_entity(mut self, entity: Entity) -> Self {
        if self.entities.is_none() {
            self.entities = Some(vec![])
        }
        let mut curr_ents = self.entities.unwrap();
        curr_ents.push(entity);
        self.entities = Some(curr_ents);
        self
    }

    pub fn add_authenticator(mut self, a: Authenticator) -> Self {
        if self.authenticators.is_none() {
            self.authenticators = Some(vec![])
        }
        let mut curr_auth = self.authenticators.unwrap();
        curr_auth.push(a);
        self.authenticators = Some(curr_auth);
        self
    }

    pub fn validate(&mut self) -> bool {
        if self.clone().entities.is_none() {
            return false
        }
        return self.clone().identifier_type.is_some() && self.clone().value.is_some() && self.clone().entities.unwrap().len() > 0
    }
}

pub struct IdentifierStore {}

static TEMPLATE_ENGINE_IDENT_STORE: OnceCell<handlebars::Handlebars> = OnceCell::new();

impl IdentifierStore {
    pub fn create(i: Identifier, fields: Vec<String>) -> Result<Option<Identifier>, failure::Error> {
        let create_ident_type = &i.clone().identifier_type.ok_or(IdentifierError::EmptyField("sid".to_string()))?;
        let create_ident_value = &i.clone().value.ok_or(IdentifierError::EmptyField("sid".to_string()))?;
        if !EntityStore::exists(
            i.clone().entities
                .ok_or(IdentifierError::Empty())?
                .get(0)
                .ok_or(IdentifierError::Empty())?
                .uid.as_ref().ok_or(IdentifierError::Empty())?
        ) {
            return Err(IdentifierError::DoesNotExist().into())
        }
        let exists: Option<Identifier> = Self::find_by_type_value(
            create_ident_type,
            &format!("^{}$", create_ident_value),
            vec!["uid".to_string()]
        )?;
        if exists.is_some() {
            return Err(IdentifierError::IdentifierExists().into())
        }
        if i.clone().validate() {
            let res = db::save(serde_json::to_vec(&i)?)?;
            let mut ass = i.clone();
            for (_, uid) in res.uids {
                ass.uid = Some(uid);
                break;
            }
            EntityStore::associate_identity(i.clone().entities
                                                .ok_or(IdentifierError::Empty())?
                                                .get(0)
                                                .ok_or(IdentifierError::Empty())?
                                                .uid.as_ref().ok_or(IdentifierError::Empty())?
                                            , ass, vec!["uid".to_string()]);
            return Self::find_by_type_value(&create_ident_type, &create_ident_value, fields)
        }
        Err(IdentifierError::IdentifierExists().into())
    }

    pub fn exists(uid: &str) -> bool {
        let exists = Self::find_by_uid(
            uid,
            vec!["uid".to_string()]
        );
        if exists.is_err() {
            return true
        }
        return exists.unwrap().is_some();
    }

    pub fn find_by_uid(uid: &str, fields: Vec<String>) -> Result<Option<Identifier>, failure::Error> {
        let reg = TEMPLATE_ENGINE_IDENT_STORE.get_or_init(|| {
            handlebars::Handlebars::new()
        });
        let req: &'static str = r#"
            query identifier($uid: string) {
			    identifier(func: uid($uid)) @filter(eq(dgraph.type, "Identifier")) {
			    {{#each fields }}
				    {{this}}
			    {{/each}}
			}
		}
        "#;
        let template_vars = &json!({
            "fields": fields
        });
        let query = reg.render_template(req, template_vars)?;
        let mut vars: HashMap<String, String> = [
            ("$uid".to_string(), uid.to_string())
        ].iter().cloned().collect();
        let res = db::query(query, vars)?;
        let e: IdentifierRoot = serde_json::from_slice(&res.json)?;
        match e.identifier.len() {
            0 => Ok(None),
            _ => Ok(Some(e.identifier.get(0).ok_or(IdentifierError::Empty())?.clone()))
        }
    }

    pub fn find_by_type_value(identifier_type: &IdentifierType, value: &String, fields: Vec<String>) -> Result<Option<Identifier>, failure::Error> {
        let reg = TEMPLATE_ENGINE_IDENT_STORE.get_or_init(|| {
            handlebars::Handlebars::new()
        });
        let req: &'static str = r#"
            query identifier($type: string, $value: string) {
			    identifier(func: regexp(value, $value)) @filter(eq(identifier_type, $type) AND eq(dgraph.type, "Identifier")) {
			    {{#each fields }}
				    {{this}}
			    {{/each}}
			}
		}
        "#;
        let template_vars = &json!({
            "fields": fields
        });
        let query = reg.render_template(req, template_vars)?;
        let mut vars: HashMap<String, String> = [
            ("$type".to_string(), identifier_type.to_string()),
            ("$value".to_string(), format!("/{}/", value))
        ].iter().cloned().collect();
        println!("{:?}", vars);
        let res = db::query(query, vars)?;
        let e: IdentifierRoot = serde_json::from_slice(&res.json)?;
        match e.identifier.len() {
            0 => Ok(None),
            _ => Ok(Some(e.identifier.get(0).ok_or(IdentifierError::Empty())?.clone()))
        }
    }

    pub fn associate_authenticator(uid: &str, e: Authenticator, fields: Vec<String>) -> Result<Option<Identifier>, failure::Error> {
        let res = Self::find_by_uid(uid, vec!["uid".to_string(), "guid".to_string(), "authenticator { uid }".to_string()])?;
        if res.is_none() {
            return Err(IdentifierError::DoesNotExist().into())
        }
        let update: Identifier = res.clone().ok_or(EntityError::Empty())?.add_authenticator(e.clone());
        println!("DEBUG => {:?}", update);
        db::save(serde_json::to_vec(&update)?)?;

        AuthenticatorStore::associate_identifier(e.uid.as_ref().ok_or(EntityError::EmptyField("uid".to_string()))?, update, vec!["uid".to_string()])?;

        return Self::find_by_uid(uid, fields);
    }
}