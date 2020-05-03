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
use crate::system::{System, SystemError, SystemStore};
use crate::scope::Scope;

#[derive(Debug, Fail)]
pub enum NamespaceError {
    #[fail(display = "Cannot save because failed validation. You probably forgot to add a name to your system")]
    ValidationFailed(),
    #[fail(display = "Cannot extract system value from an empty array or None value")]
    Empty(),
    #[fail(display = "System with guid does not exist")]
    DoesNotExist(),
    #[fail(display = "Namespace with name already exists on the system")]
    AlreadyExists()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NamespaceRoot {
    pub namespace: Vec<Namespace>
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Namespace {
    pub uid: Option<String>,
    pub guid:Option<String>,
    #[serde(rename = "system")]
    pub systems: Option<Vec<System>>,
    #[serde(rename = "scope")]
    pub scopes: Option<Vec<Scope>>,
    pub name: Option<String>,
    #[serde(rename = "dgraph.type")]
    pub dtype: Option<Vec<String>>
}

impl Namespace{
    pub fn new() -> Namespace {
        Namespace {
            guid: Some(nanoid::nanoid!()),
            dtype: Some(vec!["Namespace".to_string()]),
            ..Default::default()
        }
    }

    pub fn uid(mut self, uid: String) -> Self {
        self.uid = Some(uid);
        self
    }

    pub fn name(mut self, uid: String) -> Self {
        self.name = Some(uid);
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

    pub fn add_scope(mut self, s: Scope) -> Self {
        if self.scopes.is_none() {
            self.scopes = Some(vec![])
        }
        let mut curr_scopes = self.scopes.unwrap();
        curr_scopes.push(s);
        self.scopes = Some(curr_scopes);
        self
    }

    pub fn validate(&mut self) -> bool {
        if self.name.is_none() {
            return false
        } else {
            let name = self.name.clone().unwrap();
            if name.len() < 1 {
                return false
            }
        }
        if self.systems.is_none() {
            return false
        } else {
            let sys = self.systems.clone().unwrap();
            if sys.len() < 1 {
                return false
            }
            for s in sys {
                if s.uid.is_none() {
                    return false
                }
            }
        }
        return self.guid.is_some()
    }
}

pub struct NamespaceStore {}

static TEMPLATE_ENGINE_NS_STORE: OnceCell<handlebars::Handlebars> = OnceCell::new();

impl NamespaceStore {
    pub fn create(a: Namespace, fields: Vec<String>) -> Result<Option<Namespace>, failure::Error> {
        if a.clone().validate() {
            let exists = Self::find_by_system_name(
                a.clone().name.as_ref().ok_or(SystemError::Empty())?,
                a.clone().systems.ok_or(SystemError::Empty())?.get(0).ok_or(SystemError::Empty())?.uid.as_ref().ok_or(SystemError::Empty())?,
                vec!["uid".to_string()])?;
            if exists.is_some() {
                return Err(NamespaceError::AlreadyExists().into())
            }
            println!("DEBUG {:?}", a.clone());
            let mut tmp = a.clone();
            let res: HashMap<String, String> = db::save(serde_json::to_vec(&a)?).unwrap().uids;
            for (_, r) in res {
                tmp.uid = Some(r);
                break
            }
            println!("GUID {:?}", a.clone().systems.ok_or(SystemError::Empty())?.get(0).ok_or(SystemError::Empty())?.guid.as_ref().ok_or(SystemError::Empty())?);
            SystemStore::associate_namespace(
                a.clone().systems.ok_or(SystemError::Empty())?.get(0).ok_or(SystemError::Empty())?.guid.as_ref().ok_or(SystemError::Empty())?,
                tmp,
                vec!["uid".to_string()])?;
            return Self::find_by_guid(&a.clone().guid.ok_or(NamespaceError::Empty())?, fields)
        }
        Err(NamespaceError::ValidationFailed().into())
    }

    pub fn find_by_guid(guid: &str, fields: Vec<String>) -> Result<Option<Namespace>, failure::Error> {
        let reg = TEMPLATE_ENGINE_NS_STORE.get_or_init(|| {
            handlebars::Handlebars::new()
        });
        let req: &'static str = r#"
            query namespace($guid: string) {
			    namespace(func: eq(guid, $guid)) @filter(eq(dgraph.type, "Namespace")) {
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
        let e: NamespaceRoot = serde_json::from_slice(&res.json)?;
        match e.namespace.len() {
            0 => Ok(None),
            _ => Ok(Some(e.namespace.get(0).ok_or(SystemError::Empty())?.clone()))
        }
    }

    pub fn associate_scope(guid: &str, e: Scope, fields: Vec<String>) -> Result<Option<Namespace>, failure::Error> {
        let res = Self::find_by_guid(guid, vec!["uid".to_string(), "guid".to_string(), "namespace { uid }".to_string()])?;
        if res.is_none() {
            return Err(SystemError::DoesNotExist().into())
        }
        let update: Namespace = res.clone().ok_or(EntityError::Empty())?.add_scope(e.clone());
        db::save(serde_json::to_vec(&update)?)?;

        return Self::find_by_guid(guid, fields);
    }

    pub fn find_by_system_name(name: &str, system_uid: &str, fields: Vec<String>) -> Result<Option<Namespace>, failure::Error> {
        let reg = TEMPLATE_ENGINE_NS_STORE.get_or_init(|| {
            handlebars::Handlebars::new()
        });
        let req: &'static str = r#"
            query namespace($sys_uid: string, $name: string) {
                system(func: uid($sys_uid)) @filter(eq(dgraph.type, "System")) {
                    namespace {
                        ns_uid as uid
                    }
                }

			    namespace(func: uid(ns_uid)) @filter(eq(name, $name) AND eq(dgraph.type, "Namespace")) {
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
        println!("QUERY {}", query);
        let mut vars: HashMap<String, String> = [
            ("$sys_uid".to_string(), system_uid.to_string()),
            ("$name".to_string(), name.to_string()),
        ].iter().cloned().collect();
        println!("VAR {:?}", vars);
        let res = db::query(query, vars)?;
        println!("RES {}", std::str::from_utf8(&res.json)?);
        let e: NamespaceRoot = serde_json::from_slice(&res.json)?;
        match e.namespace.len() {
            0 => Ok(None),
            _ => Ok(Some(e.namespace.get(0).ok_or(NamespaceError::Empty())?.clone()))
        }
    }
}