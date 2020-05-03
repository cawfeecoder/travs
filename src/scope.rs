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
use crate::namespace::{Namespace, NamespaceStore};

#[derive(Debug, Fail)]
pub enum ScopeError {
    #[fail(display = "Cannot save because failed validation. You probably forgot to add a name to your system")]
    ValidationFailed(),
    #[fail(display = "Cannot extract system value from an empty array or None value")]
    Empty(),
    #[fail(display = "System with guid does not exist")]
    DoesNotExist(),
    #[fail(display = "Namespace with name already exists on the system")]
    AlreadyExists()
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ScopeType {
    group,
    action
}

impl Display for ScopeType {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ScopeRoot {
    pub scope: Vec<Scope>
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Scope {
    pub uid: Option<String>,
    pub guid: Option<String>,
    #[serde(rename = "namespace")]
    pub namespaces: Option<Vec<Namespace>>,
    pub name: Option<String>,
    #[serde(rename = "entity")]
    pub entities: Option<Vec<Entity>>,
    pub scope_type: Option<ScopeType>,
    #[serde(rename = "dgraph.type")]
    pub dtype: Option<Vec<String>>
}

impl Scope {
    pub fn new() -> Scope {
        Scope {
            guid: Some(nanoid::nanoid!()),
            dtype: Some(vec!["Scope".to_string()]),
            ..Default::default()
        }
    }

    pub fn uid(mut self, uid: String) -> Self {
        self.uid = Some(uid);
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

    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
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

    pub fn scope_type(mut self, scope_type: ScopeType) -> Self {
        self.scope_type = Some(scope_type);
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
        if self.namespaces.is_none() {
            return false
        } else {
            let ns = self.namespaces.clone().unwrap();
            if ns.len() < 1 {
                return false
            }
            for n in ns {
                if n.uid.is_none() {
                    return false
                }
            }
        }
        return self.guid.is_some() && self.scope_type.is_some()
    }
}

pub struct ScopeStore {}

static TEMPLATE_ENGINE_SCOPE_STORE: OnceCell<handlebars::Handlebars> = OnceCell::new();

impl ScopeStore {
    pub fn create(s: Scope, fields: Vec<String>) -> Result<Option<Scope>, failure::Error> {
        if s.clone().validate() {
            let exists = Self::find_by_namespace_type_name(
                s.clone().name.as_ref().ok_or(ScopeError::Empty())?,
                s.clone().namespaces.ok_or(ScopeError::Empty())?.get(0).ok_or(ScopeError::Empty())?.uid.as_ref().ok_or(ScopeError::Empty())?,
                vec!["uid".to_string()]
            )?;
            if exists.is_some() {
                return Err(ScopeError::AlreadyExists().into())
            }
            let mut tmp = s.clone();
            let res: HashMap<String, String> = db::save(serde_json::to_vec(&s)?).unwrap().uids;
            for (_, r) in res {
                tmp.uid = Some(r);
                break
            }
            NamespaceStore::associate_scope(
                s.clone().namespaces.ok_or(ScopeError::Empty())?.get(0).ok_or(ScopeError::Empty())?.guid.as_ref().ok_or(ScopeError::Empty())?,
                tmp,
                vec!["uid".to_string()]
            )?;
            return Self::find_by_guid(&s.clone().guid.ok_or(ScopeError::Empty())?, fields)
        }
        Err(ScopeError::ValidationFailed().into())
    }

    pub fn find_by_guid(guid: &str, fields: Vec<String>) -> Result<Option<Scope>, failure::Error> {
        let reg = TEMPLATE_ENGINE_SCOPE_STORE.get_or_init(|| {
            handlebars::Handlebars::new()
        });
        let req: &'static str = r#"
            query scope($guid: string) {
			    scope(func: eq(guid, $guid)) @filter(eq(dgraph.type, "Scope")) {
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
        let e: ScopeRoot = serde_json::from_slice(&res.json)?;
        match e.scope.len() {
            0 => Ok(None),
            _ => Ok(Some(e.scope.get(0).ok_or(ScopeError::Empty())?.clone()))
        }
    }

    pub fn associate_entity(guid: &str, e: Entity, fields: Vec<String>) -> Result<Option<Scope>, failure::Error> {
        let res = Self::find_by_guid(guid, vec!["uid".to_string(), "guid".to_string(), "entity { uid }".to_string()])?;
        if res.is_none() {
            return Err(ScopeError::DoesNotExist().into())
        }
        let update: Scope = res.clone().ok_or(EntityError::Empty())?.add_entity(e.clone());
        println!("DEBUG BITCH {:?}", update);
        db::save(serde_json::to_vec(&update)?)?;

        EntityStore::associate_scope(e.uid.as_ref().ok_or(EntityError::EmptyField("uid".to_string()))?, update, vec!["uid".to_string()])?;

        return Self::find_by_guid(guid, fields);
    }

    pub fn find_by_namespace_type_name(name: &str, namespace_uid: &str, fields: Vec<String>) -> Result<Option<Scope>, failure::Error> {
        let reg = TEMPLATE_ENGINE_SCOPE_STORE.get_or_init(|| {
            handlebars::Handlebars::new()
        });
        let req: &'static str = r#"
            query scope($ns_uid: string, $name: string) {
                namespace(func: uid($ns_uid)) @filter(eq(dgraph.type, "Namespace")) {
                    scope {
                        scope_uid as uid
                    }
                }

			    scope(func: uid(scope_uid)) @filter(eq(name, $name) AND eq(dgraph.type, "Scope")) {
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
            ("$ns_uid".to_string(), namespace_uid.to_string()),
            ("$name".to_string(), name.to_string()),
        ].iter().cloned().collect();
        println!("VAR {:?}", vars);
        let res = db::query(query, vars)?;
        println!("RES {}", std::str::from_utf8(&res.json)?);
        let e: ScopeRoot = serde_json::from_slice(&res.json)?;
        match e.scope.len() {
            0 => Ok(None),
            _ => Ok(Some(e.scope.get(0).ok_or(ScopeError::Empty())?.clone()))
        }
    }
}