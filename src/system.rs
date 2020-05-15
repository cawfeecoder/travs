use serde_json::json;
use std::fmt::{Formatter, Display};
use once_cell::sync::OnceCell;
use crate::db;
use std::collections::HashMap;
use serde_derive::{Deserialize, Serialize};
use failure_derive::*;
use crate::entity::{Entity, EntityStore, EntityError};
use crate::identifier::{Identifier, IdentifierStore, IdentifierType, IdentifierError};
use crate::authenticator::{Authenticator, AuthenticatorStore};
use crate::system::SystemError::ValidationFailed;
use crate::namespace::Namespace;

#[derive(Debug, Fail)]
pub enum SystemError {
    #[fail(display = "Cannot save because failed validation. You probably forgot to add a name to your system")]
    ValidationFailed(),
    #[fail(display = "Cannot extract system value from an empty array or None value")]
    Empty(),
    #[fail(display = "System with guid does not exist")]
    DoesNotExist(),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SystemRoot {
    pub system: Vec<System>
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct System {
    pub uid: Option<String>,
    pub guid:Option<String>,
    #[serde(rename = "authenticator")]
    pub authenicators: Option<Vec<Authenticator>>,
    #[serde(rename = "entity")]
    pub entities: Option<Vec<Entity>>,
    #[serde(rename = "namespace")]
    pub namespaces: Option<Vec<Namespace>>,
    pub name: Option<String>,
    #[serde(rename = "dgraph.type")]
    pub dtype: Option<Vec<String>>
}

impl System {
    pub fn new() -> System {
        System {
            guid: Some(nanoid::nanoid!()),
            dtype: Some(vec!["System".to_string()]),
            ..Default::default()
        }
    }

    pub fn guid(mut self, guid: String) -> Self {
        self.guid = Some(guid);
        self
    }

    pub fn uid(mut self, uid: String) -> Self {
        self.uid = Some(uid);
        self
    }

    pub fn name(mut self, uid: String) -> Self {
        self.name = Some(uid);
        self
    }

    pub fn add_authenticator(mut self, a: Authenticator) -> Self {
        if self.authenicators.is_none() {
            self.authenicators = Some(vec![])
        }
        let mut curr_auth = self.authenicators.unwrap();
        curr_auth.push(a);
        self.authenicators = Some(curr_auth);
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

    pub fn add_namespace(mut self, ns: Namespace) -> Self {
        if self.namespaces.is_none() {
            self.namespaces = Some(vec![])
        }
        let mut curr_ns = self.namespaces.unwrap();
        curr_ns.push(ns);
        self.namespaces = Some(curr_ns);
        self
    }

    pub fn validate(&mut self) -> bool {
        if self.name.is_none() {
            return false
        } else {
            let name = self.name.clone().unwrap();
            if name.len() < 0 {
                return false
            }
        }
        return self.guid.is_some()
    }
}

pub struct SystemStore {}

static TEMPLATE_ENGINE_SYS_STORE: OnceCell<handlebars::Handlebars> = OnceCell::new();

impl SystemStore {
    pub fn create(a: System, fields: Vec<String>) -> Result<Option<System>, failure::Error> {
        if a.clone().validate() {
            let _ = db::save(serde_json::to_vec(&a)?).unwrap().uids;
            return Self::find_by_guid(&a.clone().guid.ok_or(SystemError::Empty())?, fields)
        }
        Err(SystemError::ValidationFailed().into())
    }

    pub fn associate_entity(guid: &str, e: Entity, fields: Vec<String>) -> Result<Option<System>, failure::Error> {
        let res = Self::find_by_guid(guid, vec!["uid".to_string(), "guid".to_string(), "entity { uid }".to_string()])?;
        if res.is_none() {
            return Err(SystemError::DoesNotExist().into())
        }
        let update: System = res.clone().ok_or(EntityError::Empty())?.add_entity(e.clone());
        db::save(serde_json::to_vec(&update)?)?;

        EntityStore::associate_system(e.uid.as_ref().ok_or(EntityError::EmptyField("uid".to_string()))?, update, vec!["uid".to_string()])?;

        return Self::find_by_guid(guid, fields);
    }

    pub fn associate_authenticator(guid: &str, e: Authenticator, fields: Vec<String>) -> Result<Option<System>, failure::Error> {
        let res = Self::find_by_guid(guid, vec!["uid".to_string(), "guid".to_string(), "authenticator { uid }".to_string()])?;
        if res.is_none() {
            return Err(SystemError::DoesNotExist().into())
        }
        let update: System = res.clone().ok_or(EntityError::Empty())?.add_authenticator(e.clone());
        db::save(serde_json::to_vec(&update)?)?;

        AuthenticatorStore::associate_system(e.uid.as_ref().ok_or(EntityError::EmptyField("uid".to_string()))?, update, vec!["uid".to_string()])?;

        return Self::find_by_guid(guid, fields);
    }

    pub fn associate_namespace(guid: &str, e: Namespace, fields: Vec<String>) -> Result<Option<System>, failure::Error> {
        let res = Self::find_by_guid(guid, vec!["uid".to_string(), "guid".to_string(), "namespace { uid }".to_string()])?;
        if res.is_none() {
            return Err(SystemError::DoesNotExist().into())
        }
        let update: System = res.clone().ok_or(EntityError::Empty())?.add_namespace(e.clone());
        db::save(serde_json::to_vec(&update)?)?;

        return Self::find_by_guid(guid, fields);
    }

    pub fn find_by_guid(guid: &str, fields: Vec<String>) -> Result<Option<System>, failure::Error> {
        let reg = TEMPLATE_ENGINE_SYS_STORE.get_or_init(|| {
            handlebars::Handlebars::new()
        });
        let req: &'static str = r#"
            query system($guid: string) {
			    system(func: eq(guid, $guid)) @filter(eq(dgraph.type, "System")) {
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
            ("$guid".to_string(), guid.to_string())
        ].iter().cloned().collect();
        let res = db::query(query, vars)?;
        let e: SystemRoot = serde_json::from_slice(&res.json)?;
        match e.system.len() {
            0 => Ok(None),
            _ => Ok(Some(e.system.get(0).ok_or(SystemError::Empty())?.clone()))
        }
    }
}




