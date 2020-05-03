use serde_json::json;
use once_cell::sync::OnceCell;
use crate::db;
use std::collections::HashMap;
use serde_derive::{Deserialize, Serialize};
use failure_derive::*;
use crate::entity::EntityError::SidExistsError;
use crate::identifier::*;
use failure::_core::convert::identity;
use failure::_core::hash::Hash;
use crate::identifier::{IdentifierRoot, Identifier, IdentifierStore, IdentifierType};
use crate::authenticator::{Authenticator, AuthenticatorStore};
use crate::system::System;
use crate::scope::Scope;

#[derive(Debug, Fail)]
pub enum EntityError {
    #[fail(display = "An entity with sid already exists")]
    SidExistsError(),
    #[fail(display = "Entity cannot not have empty")]
    EmptyField(String),
    #[fail(display = "Entity with uid does not exist")]
    DoesNotExist(),
    #[fail(display = "Cannot extract entity value from an empty array or None value")]
    Empty()
}

impl From<String> for EntityError {
    fn from(e: String) -> Self {
        EntityError::EmptyField(e)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EntityRoot {
    pub entity: Vec<Entity>
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Entity {
    pub uid: Option<String>,
    pub guid:Option<String>,
    pub sid: Option<String>,
    pub display_name: Option<String>,
    #[serde(rename = "identifier")]
    pub identifiers: Option<Vec<Identifier>>,
    #[serde(rename = "authenticator")]
    pub authenicators: Option<Vec<Authenticator>>,
    #[serde(rename = "system")]
    pub systems: Option<Vec<System>>,
    #[serde(rename = "scope")]
    pub scopes: Option<Vec<Scope>>,
    #[serde(rename = "dgraph.type")]
    pub dtype: Option<Vec<String>>
}

impl Entity {
    pub fn new() -> Entity {
        Entity {
            guid: Some(nanoid::nanoid!()),
            dtype: Some(vec!["Entity".to_string()]),
            ..Default::default()
        }
    }

    pub fn uid(mut self, uid: String) -> Self {
        self.uid = Some(uid);
        self
    }

    pub fn display_name(mut self, display_name: String) -> Self {
        self.display_name = Some(display_name);
        self
    }

    pub fn sid(mut self, sid: String) -> Self {
        self.sid = Some(sid);
        self
    }

    pub fn add_identifier(mut self, i: Identifier) -> Self {
        if self.identifiers.is_none() {
            self.identifiers = Some(vec![])
        }
        let mut curr_idents = self.identifiers.unwrap();
        curr_idents.push(i);
        self.identifiers = Some(curr_idents);
        self
    }

    pub fn add_authenticator(mut self, a: Authenticator) -> Self {
        if self.authenicators.is_none() {
            self.authenicators = Some(vec![])
        }
        let mut curr_auths = self.authenicators.unwrap();
        curr_auths.push(a);
        self.authenicators = Some(curr_auths);
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
        let mut curr_scs = self.scopes.unwrap();
        curr_scs.push(s);
        self.scopes = Some(curr_scs);
        self
    }

    pub fn validate(&mut self) -> bool {
        return self.display_name.is_some() && self.sid.is_some() && self.guid.is_some()
    }
}

pub struct EntityStore {}

static TEMPLATE_ENGINE_ENTITY_STORE: OnceCell<handlebars::Handlebars> = OnceCell::new();

impl EntityStore {
    // fn _create_idents(e: Entity, uids: HashMap<String, String>) -> Result<Entity, failure::Error> {
    //     let mut ent = Entity::new();
    //     for (_, uid) in uids.iter() {
    //         ent.uid = Some(uid.to_string());
    //         break;
    //     }
    //     let mut identifiers: Vec<Identifier> = vec![];
    //     for mut ident in e.clone().identifiers.unwrap() {
    //         if ident.clone().uid.is_some() {
    //             identifiers.push(ident.clone());
    //             continue
    //         }
    //         let mut create_ident = ident.clone();
    //         create_ident.entities = Some(vec![]);
    //         let create_ident = create_ident.add_entity(ent.clone());
    //         match IdentifierStore::create(create_ident, vec!["uid".to_string()]) {
    //             Ok(v) => {
    //                 identifiers.push(v.unwrap());
    //             }
    //             Err(err) => {
    //                 return Err(err.into())
    //             }
    //         };
    //     }
    //     ent.identifiers = Some(identifiers);
    //     Ok(ent)
    // }
    //
    // fn _create_auths(e: Entity, uids: HashMap<String, String>) -> Result<Entity, failure::Error> {
    //     let mut ent = Entity::new();
    //     for (_, uid) in uids.iter() {
    //         ent.uid = Some(uid.to_string());
    //         break;
    //     }
    //     let mut authenticators: Vec<Authenticator> = vec![];
    //     for mut auth in e.clone().authenicators.unwrap() {
    //         if auth.clone().uid.is_some() {
    //             authenticators.push(auth.clone());
    //             continue
    //         }
    //         let mut create_auth = auth.clone();
    //         create_auth.entities = Some(vec![]);
    //         let create_auth = create_auth.add_entity(ent.clone());
    //         match AuthenticatorStore::create(create_auth, vec!["uid".to_string()]) {
    //             Ok(v) => {
    //                 authenticators.push(v.unwrap());
    //             }
    //             Err(err) => {
    //                 return Err(err.into())
    //             }
    //         };
    //     }
    //     ent.authenicators = Some(authenticators);
    //     Ok(ent)
    // }

    pub fn create(mut e: Entity, fields: Vec<String>) -> Result<Option<Entity>, failure::Error> {
        let create_sid = &e.clone().sid.ok_or(EntityError::EmptyField("sid".to_string()))?;
        let exists: Option<Entity> = Self::find_by_sid(
            create_sid,
            vec!["uid".to_string()]
        )?;
        if exists.is_none() && e.validate() {
            // let mut tmp = e.clone();
            e.identifiers = None;
            e.authenicators = None;
            let _ = db::save(serde_json::to_vec(&e)?).unwrap().uids;
            // if tmp.clone().identifiers.is_some() {
            //     tmp = EntityStore::_create_idents(tmp.clone(), res.clone())?;
            // }
            // if tmp.clone().authenicators.is_some() {
            //     tmp = EntityStore::_create_auths(tmp.clone(), res.clone())?;
            // }
            // if tmp.clone().identifiers.is_some() || tmp.clone().authenicators.is_some() {
            //     db::save(serde_json::to_vec(&tmp)?);
            // }
            return EntityStore::find_by_sid(create_sid, fields)
        }
        Err(SidExistsError().into())
    }

    pub fn associate_identity(uid: &str, i: Identifier, fields: Vec<String>) -> Result<Option<Entity>, failure::Error> {
        let res = Self::find_by_uid(uid, vec!["uid".to_string(), "identifier { uid }".to_string()])?;
        if res.is_none() {
            return Err(EntityError::DoesNotExist().into())
        }
        let update: Entity = res.clone().ok_or(EntityError::Empty())?.add_identifier(i);
        db::save(serde_json::to_vec(&update)?)?;
        return Self::find_by_uid(uid, fields);
    }

    pub fn associate_authenticator(uid: &str, a: Authenticator, fields: Vec<String>) -> Result<Option<Entity>, failure::Error> {
        let res = Self::find_by_uid(uid, vec!["uid".to_string(), "authenticator { uid }".to_string()])?;
        if res.is_none() {
            return Err(EntityError::DoesNotExist().into())
        }
        let update: Entity = res.clone().ok_or(EntityError::Empty())?.add_authenticator(a);
        db::save(serde_json::to_vec(&update)?)?;
        return Self::find_by_uid(uid, fields);
    }

    pub fn associate_system(uid: &str, s: System, fields: Vec<String>) -> Result<Option<Entity>, failure::Error> {
        let res = Self::find_by_uid(uid, vec!["uid".to_string(), "system { uid }".to_string()])?;
        if res.is_none() {
            return Err(EntityError::DoesNotExist().into())
        }
        let update: Entity = res.clone().ok_or(EntityError::Empty())?.add_system(s);
        println!("DEBUG {:?}", update);
        db::save(serde_json::to_vec(&update)?)?;

        return Self::find_by_uid(uid, fields);
    }

    pub fn associate_scope(uid: &str, s: Scope, fields: Vec<String>) -> Result<Option<Entity>, failure::Error> {
        let res = Self::find_by_uid(uid, vec!["uid".to_string(), "scope { uid }".to_string()])?;
        if res.is_none() {
            return Err(EntityError::DoesNotExist().into())
        }
        let update: Entity = res.clone().ok_or(EntityError::Empty())?.add_scope(s);
        println!("DEBUG {:?}", update);
        db::save(serde_json::to_vec(&update)?)?;

        return Self::find_by_uid(uid, fields);
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

    pub fn find_by_uid(uid: &str, fields: Vec<String>) -> Result<Option<Entity>, failure::Error> {
        let reg = TEMPLATE_ENGINE_ENTITY_STORE.get_or_init(|| {
            handlebars::Handlebars::new()
        });
        let req: &'static str = r#"
            query entity($uid: string) {
			    entity(func: uid($uid)) @filter(eq(dgraph.type, "Entity")) {
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
        let e: EntityRoot = serde_json::from_slice(&res.json)?;
        match e.entity.len() {
            0 => Ok(None),
            _ => Ok(Some(e.entity.get(0).ok_or(EntityError::Empty())?.clone()))
        }
    }

    pub fn find_by_sid(sid: &str, fields: Vec<String>) -> Result<Option<Entity>, failure::Error> {
        let reg = TEMPLATE_ENGINE_ENTITY_STORE.get_or_init(|| {
            handlebars::Handlebars::new()
        });
        let req: &'static str = r#"
            query entity($id: string) {
			    entity(func: eq(sid, $id)) {
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
            ("$id".to_string(), sid.to_string())
        ].iter().cloned().collect();
        let res = db::query(query, vars)?;
        let e: EntityRoot = serde_json::from_slice(&res.json)?;
        match e.entity.len() {
            0 => Ok(None),
            _ => Ok(Some(e.entity.get(0).ok_or(EntityError::Empty())?.clone()))
        }
    }

    pub fn find_by_identifier(identifier_type: IdentifierType, value: String, fields: Vec<String>) -> Result<Option<Entity>, failure::Error> {
        let reg = TEMPLATE_ENGINE_ENTITY_STORE.get_or_init(|| {
            handlebars::Handlebars::new()
        });
        let req: &'static str = r#"
            query identifer($type: string, $value: string) {
			    identifier(func: regexp(value, $value)) @filter(eq(identifier_type, $type) AND eq(dgraph.type, "Identifier")) {
			        entity {
			        {{#each fields }}
				        {{this}}
			        {{/each}}
			        }
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
        let res = db::query(query, vars)?;
        let e: IdentifierRoot = serde_json::from_slice(&res.json)?;

        match e.identifier.len() {
            0 => Ok(None),
            _ => Ok(Some(
                e.identifier
                    .get(0)
                    .ok_or(EntityError::Empty())?
                    .clone()
                    .entities
                    .ok_or(EntityError::Empty())?
                    .get(0)
                    .ok_or(EntityError::Empty())?
                    .clone()))
        }
    }
}