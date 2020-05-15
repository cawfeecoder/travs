use serde_json::json;
use std::fmt::{Formatter, Display};
use once_cell::sync::OnceCell;
use crate::db;
use std::collections::HashMap;
use serde_derive::{Deserialize, Serialize};
use failure_derive::*;
use crate::entity::{Entity, EntityStore, EntityError};
use crate::identifier::{Identifier, IdentifierStore, IdentifierType, IdentifierError};
use crate::system::{System, SystemStore};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuthenticatorRoot {
    pub authenticator: Vec<Authenticator>
}

#[derive(Debug, Fail)]
pub enum AuthenticatorError {
    #[fail(display = "An authenticator with of this type exists and is associated to this identity")]
    AuthenticatorExists(),
    #[fail(display = "Field {} cannot be empty", 0)]
    EmptyField(String),
    #[fail(display = "Authenticator type does not match the identifier type being associated")]
    AuthTypeIdentTypeMisMatch(),
    #[fail(display = "Cannot extract authenticator value from an empty array or None value")]
    Empty(),
    #[fail(display = "Authenticator with uid does not exist")]
    DoesNotExist(),
}

impl From<String> for AuthenticatorError {
    fn from(e: String) -> Self {
        AuthenticatorError::EmptyField(e)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AuthenticatorType {
    username_password,
    phone_password,
    email_password,
    public_key_authentication

}

impl Display for AuthenticatorType {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}


#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Authenticator {
    pub uid: Option<String>,
    #[serde(rename = "identifier")]
    pub identifiers: Option<Vec<Identifier>>,
    #[serde(rename = "entity")]
    pub entities: Option<Vec<Entity>>,
    #[serde(rename = "system")]
    pub systems: Option<Vec<System>>,
    pub authenticator_type: Option<AuthenticatorType>,
    pub value: Option<String>,
    #[serde(rename = "dgraph.type")]
    pub dtype: Option<Vec<String>>
}

impl Authenticator {
    pub fn new() -> Authenticator {
        Authenticator {
            dtype: Some(vec!["Authenticator".to_string()]),
            ..Default::default()
        }
    }

    pub fn uid(mut self, uid: String) -> Self {
        self.uid = Some(uid);
        self
    }

    pub fn authenticator_type(mut self, authenticator_type: AuthenticatorType) -> Self {
        self.authenticator_type = Some(authenticator_type);
        self
    }

    pub fn add_identifier(mut self, identifier: Identifier) -> Self {
        if self.identifiers.is_none() {
            self.identifiers = Some(vec![])
        }
        let mut curr_ident = self.identifiers.unwrap();
        curr_ident.push(identifier);
        self.identifiers = Some(curr_ident);
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

    pub fn add_system(mut self, s: System) -> Self {
        if self.systems.is_none() {
            self.systems = Some(vec![])
        }
        let mut curr_sys = self.systems.unwrap();
        curr_sys.push(s);
        self.systems = Some(curr_sys);
        self
    }

    pub fn value(mut self, value: String) -> Self {
        self.value = Some(value);
        self
    }

    pub fn validate(&mut self) -> bool {
        if self.entities.is_none() || self.identifiers.is_none() {
            return false
        }
        return self.authenticator_type.is_some() && self.value.is_some()
    }
}

pub struct AuthenticatorStore {}

static TEMPLATE_ENGINE_AUTH_STORE: OnceCell<handlebars::Handlebars> = OnceCell::new();

impl AuthenticatorStore {
    pub fn _validate_authenticator_type(auth_type: &AuthenticatorType, ident_type: &IdentifierType) -> bool {
        return match auth_type {
            AuthenticatorType::phone_password => {
                ident_type == &IdentifierType::phone
            },
            AuthenticatorType::public_key_authentication => {
                ident_type == &IdentifierType::public_key
            },
            AuthenticatorType::email_password => {
                ident_type == &IdentifierType::email
            },
            AuthenticatorType::username_password => {
                ident_type == &IdentifierType::username
            }
        }
    }

    pub fn create(a: Authenticator, fields: Vec<String>) -> Result<Option<Authenticator>, failure::Error> {
        let create_auth_type = a.clone()
            .authenticator_type
            .ok_or(AuthenticatorError::EmptyField("authenticator_type".to_string()))?.clone();
        let mut create_auth_ident = a.clone().
            identifiers.
            ok_or(AuthenticatorError::Empty())?
            .clone()
            .get(0)
            .ok_or(AuthenticatorError::Empty())?
            .clone();
        if !Self::_validate_authenticator_type(&a.clone()
            .authenticator_type
            .ok_or(AuthenticatorError::Empty())?
                                               , &create_auth_ident.clone().identifier_type.ok_or(AuthenticatorError::Empty())?
        ) {
            return Err(AuthenticatorError::AuthTypeIdentTypeMisMatch().into())
        }
        if !EntityStore::exists(
            a.clone().entities
            .ok_or(AuthenticatorError::Empty())?
            .get(0)
            .ok_or(AuthenticatorError::Empty())?
            .uid.as_ref().ok_or(AuthenticatorError::Empty())?
        ) {
            return Err(AuthenticatorError::DoesNotExist().into())
        }
        if !IdentifierStore::exists(
            &a.clone().identifiers
                .ok_or(AuthenticatorError::Empty())?
                .get(0)
                .ok_or(AuthenticatorError::Empty())?
                .uid.as_ref().ok_or(AuthenticatorError::Empty())?
        ) {
            return Err(AuthenticatorError::DoesNotExist().into())
        }
        if create_auth_ident.identifier_type.is_none() {
            create_auth_ident = IdentifierStore::find_by_uid(
                &create_auth_ident.uid.ok_or(AuthenticatorError::Empty())?,
                vec!["uid".to_string(),
                     "identifier_type".to_string()])?.ok_or(AuthenticatorError::Empty()
            )?
        }
        let exists = Self::find_by_type_identifier(
            &create_auth_type,
            &create_auth_ident,
            vec!["uid".to_string()]
        )?;
        if exists.is_some() {
            return Err(AuthenticatorError::AuthenticatorExists().into())
        }
        if a.clone().validate() {
            let res = db::save(serde_json::to_vec(&a)?)?;
            let mut ass = a.clone();
            for (_, uid) in res.uids {
                ass.uid = Some(uid);
                break;
            }
            EntityStore::associate_authenticator(a.clone().entities
                                                .ok_or(AuthenticatorError::Empty())?
                                                .get(0)
                                                .ok_or(AuthenticatorError::Empty())?
                                                .uid.as_ref().ok_or(AuthenticatorError::Empty())?
                                            , ass.clone(), vec!["uid".to_string()]);
            SystemStore::associate_authenticator(a.clone().systems
                                                     .ok_or(AuthenticatorError::Empty())?
                                                     .get(0)
                                                     .ok_or(AuthenticatorError::Empty())?
                                                     .guid.as_ref().ok_or(AuthenticatorError::Empty())?
                                                 , ass.clone(), vec!["uid".to_string()]);
            IdentifierStore::associate_authenticator(a.clone().identifiers
                                                         .ok_or(AuthenticatorError::Empty())?
                                                         .get(0)
                                                         .ok_or(AuthenticatorError::Empty())?
                                                         .uid.as_ref().ok_or(AuthenticatorError::Empty())?
                                                     , ass.clone(), vec!["uid".to_string()]);
            return Self::find_by_type_identifier(&create_auth_type, &create_auth_ident, fields)
        }
        Err(AuthenticatorError::AuthenticatorExists().into())
    }

    pub fn associate_system(uid: &str, s: System, fields: Vec<String>) -> Result<Option<Authenticator>, failure::Error> {
        let res = Self::find_by_uid(uid, vec!["uid".to_string(), "system { uid }".to_string()])?;
        if res.is_none() {
            return Err(EntityError::DoesNotExist().into())
        }
        let update: Authenticator = res.clone().ok_or(AuthenticatorError::Empty())?.add_system(s);
        db::save(serde_json::to_vec(&update)?)?;

        return Self::find_by_uid(uid, fields);
    }

    pub fn associate_identifier(uid: &str, s: Identifier, fields: Vec<String>) -> Result<Option<Authenticator>, failure::Error> {
        let res = Self::find_by_uid(uid, vec!["uid".to_string(), "system { uid }".to_string()])?;
        if res.is_none() {
            return Err(EntityError::DoesNotExist().into())
        }
        let update: Authenticator = res.clone().ok_or(AuthenticatorError::Empty())?.add_identifier(s);
        db::save(serde_json::to_vec(&update)?)?;

        return Self::find_by_uid(uid, fields);
    }

    pub fn find_by_type_identifier(authenticator_type: &AuthenticatorType, i: &Identifier, fields: Vec<String>) -> Result<Option<Authenticator>, failure::Error> {
        if i.uid.is_none() {
            return Err(AuthenticatorError::EmptyField("uid".to_string()).into())
        }
        let reg = TEMPLATE_ENGINE_AUTH_STORE.get_or_init(|| {
            handlebars::Handlebars::new()
        });
        let req: &'static str = r#"
        query authenticator($type: string, $uid: string) {
			    authenticator(func: eq(authenticator_type, $type)) @filter(eq(dgraph.type, "Authenticator")) {
			        {{#each fields }}
			          {{this}}
			        {{/each}}
			        _: identifier @filter(uid($uid)) {
			            uid
			        }
			}
		}
        "#;
        let template_vars = &json!({
            "fields": fields
        });
        let query = reg.render_template(req, template_vars)?;
        let mut vars: HashMap<String, String> = [
            ("$type".to_string(), authenticator_type.to_string()),
            ("$uid".to_string(), format!("{}", i.uid.as_ref().ok_or(AuthenticatorError::Empty())?))
        ].iter().cloned().collect();
        let res = db::query(query, vars)?;
        let e: AuthenticatorRoot = serde_json::from_slice(&res.json)?;
        match e.authenticator.len() {
            0 => Ok(None),
            _ => Ok(Some(e.authenticator.get(0).ok_or(AuthenticatorError::Empty())?.clone()))
        }
    }

    pub fn login(a: Authenticator, i: Identifier, s: System) -> Result<bool, failure::Error> {
        let reg = TEMPLATE_ENGINE_AUTH_STORE.get_or_init(|| {
            handlebars::Handlebars::new()
        });
        let req: &'static str = r#"
            query authenticator($auth_type: string, $ident_type: string, $ident_value: string, $sys_guid: string) {
                identifier(func: eq(identifier_type, $ident_type)) @filter(eq(value, $ident_value)) {
                    A as authenticator @filter(eq(authenticator_type, $auth_type))
                }

                authenticator(func: uid(A)) {
                    value
                    system @filter(eq(guid, $sys_guid)) {
                        uid
                    }
                }
			}
        "#;
        let template_vars = &json!({});
        let query = reg.render_template(req, template_vars)?;
        let mut vars: HashMap<String, String> = [
            ("$auth_type".to_string(), format!("{}", a.authenticator_type.as_ref().ok_or(AuthenticatorError::Empty())?)),
            ("$ident_type".to_string(), format!("{}", i.identifier_type.as_ref().ok_or(AuthenticatorError::Empty())?)),
            ("$ident_value".to_string(), format!("{}", i.value.as_ref().ok_or(AuthenticatorError::Empty())?)),
            ("$sys_guid".to_string(), format!("{}", s.guid.as_ref().ok_or(AuthenticatorError::Empty())?))
        ].iter().cloned().collect();
        let res = db::query(query, vars)?;
        let e: AuthenticatorRoot = serde_json::from_slice(&res.json)?;
        match e.authenticator.len() {
            0 => return Ok(false),
            _ => {
                let extracted_auth = e.authenticator.get(0).ok_or(AuthenticatorError::Empty())?.clone();
                if extracted_auth.systems.is_none() {
                    return Ok(false)
                }
                let cmp = argon2::verify_encoded(extracted_auth.value.unwrap().as_str(), a.clone().value.unwrap().as_bytes());
                match cmp {
                    Ok(v) => {
                        Ok(v)
                    }
                    Err(e) => {
                        Err(e.into())
                    }
                }
            }
        }
    }

    pub fn find_by_uid(uid: &str, fields: Vec<String>) -> Result<Option<Authenticator>, failure::Error> {
        let reg = TEMPLATE_ENGINE_AUTH_STORE.get_or_init(|| {
            handlebars::Handlebars::new()
        });
        let req: &'static str = r#"
            query authenticator($uid: string) {
			    authenticator(func: uid($uid)) @filter(eq(dgraph.type, "Authenticator")) {
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
        let e: AuthenticatorRoot = serde_json::from_slice(&res.json)?;
        match e.authenticator.len() {
            0 => Ok(None),
            _ => Ok(Some(e.authenticator.get(0).ok_or(AuthenticatorError::Empty())?.clone()))
        }
    }
}