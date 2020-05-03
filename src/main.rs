extern crate dgraph;
extern crate once_cell;
extern crate nanoid;
extern crate handlebars;
extern crate serde_derive;

use crate::identifier::{IdentifierError, IdentifierType};
use crate::entity::EntityError;
use crate::system::SystemError;
use crate::authenticator::AuthenticatorError;
use crate::namespace::NamespaceError;
use crate::scope::ScopeError;

mod system;
mod authenticator;
mod identifier;
mod entity;
mod db;
mod scope;
mod namespace;

fn main() -> Result<(), failure::Error> {
    db::drop_all();
    let schema = r#"
        display_name: string @index(trigram) .
		guid: string @index(exact) .
		sid: string @index(exact) .
		value: string @index(trigram, exact) .
		identifier_type: string @index(exact) .
		authenticator_type: string @index(exact) .
		identifier: [uid] @reverse .
		entity: [uid] @reverse .
		authenticator: [uid] @reverse .
		name: string @index(trigram, exact) .
		system: [uid] @reverse .
		scope_type: string @index(exact) .
		scope: [uid] @reverse .
		namespace: [uid] @reverse .

		type Entity {
			guid
			sid
			display_name
			identifier
			authenticator
			system
			scope
		}

		type Identifier {
		    entity
			identifier_type
			value
		}

		type Authenticator {
		    entity
		    identifier
		    authenticator_type
		    value
		}

		type System {
		    guid
		    name
		    entity
		    authenticator
		    namespace
		}

		type Namespace {
		    guid
		    name
		    system
		    scope
		}

		type Scope {
		    guid
		    name
		    namespace
		    scope_type
		    entity
		}
    "#;
    db::migrate_schema(schema)?;
    let mut test_e = entity::Entity::new().display_name("Test Testy".to_string()).sid("testy".to_string());
    let res_create = match entity::EntityStore::create(test_e.clone(), vec!["uid".to_string()]){
        Ok(v) => v,
        Err(e) => {
            println!("{}", e);
            return Ok(())
        }
    };
    test_e.uid = res_create.ok_or(EntityError::Empty())?.uid;
    let mut test_ident = identifier::Identifier::new().identifier_type(identifier::IdentifierType::email).value("test@test.com".to_string()).add_entity(test_e.clone());
    let res_create = match identifier::IdentifierStore::create(test_ident.clone(),vec!["uid".to_string()]) {
        Ok(v) => v,
        Err(e) => {
            println!("{}", e);
            return Ok(())
        }
    };
    test_ident.uid = res_create.ok_or(IdentifierError::Empty())?.uid;
    let res_search = match entity::EntityStore::find_by_sid("testy", vec!["uid".to_string()]) {
        Ok(v) => v,
        Err(e) => {
            println!("{}", e);
            return Ok(())
        }
    };
    println!("Res Search => {:?}", res_search);
    let search_by_identifier = match entity::EntityStore::find_by_identifier(identifier::IdentifierType::email, "test@test.com".to_string(), vec!["uid".to_string(), "identifier { uid }".to_string()]) {
        Ok(v) => {
            println!("Search Results => {:?}", v);
            v
        },
        Err(e) => {
            println!("{}", e);
            return Ok(())
        }
    };
    let password = bcrypt::hash("test123", bcrypt::DEFAULT_COST)?;
    let mut test_a = authenticator::Authenticator::new()
        .authenticator_type(authenticator::AuthenticatorType::email_password)
        .value(password)
        .add_entity(test_e.clone())
        .add_identifier(test_ident.clone());
    let create_auth = match authenticator::AuthenticatorStore::create(test_a.clone(), vec!["uid".to_string()]) {
        Ok(v) => {
            println!("Create Result => {:?}", v);
            v
        },
        Err(e) => {
            println!("{:?}", e);
            return Ok(())
        }
    };
    test_a.uid = create_auth.ok_or(AuthenticatorError::Empty())?.uid;
    let mut test_system = system::System::new().name("test".to_string());
    let create_auth = match system::SystemStore::create(test_system.clone(), vec!["uid".to_string()]) {
        Ok(v) => {
            println!("Create Result => {:?}", v);
            v
        },
        Err(e) => {
            println!("{:?}", e);
            return Ok(())
        }
    };
    test_system.uid = create_auth.ok_or(SystemError::Empty())?.uid;
    let test_assoc = match system::SystemStore::associate_entity(test_system.clone().guid.as_ref().ok_or(SystemError::Empty())?, test_e.clone(), vec!["uid".to_string(), "entity { uid }".to_string()]) {
        Ok(v) => println!("Assoc Result => {:?}", v),
        Err(e) => {
            println!("{:?}", e);
            return Ok(())
        }
    };
    println!("{:?}", test_a.clone());
    let test_assoc_2 = match system::SystemStore::associate_authenticator(test_system.clone().guid.as_ref().ok_or(SystemError::Empty())?, test_a.clone(), vec!["uid".to_string(), "authenticator { uid }".to_string()]) {
        Ok(v) => println!("Assoc 2 Result => {:?}", v),
        Err(e) => {
            println!("{:?}", e);
        }
    };
    let mut test_ns = namespace::Namespace::new().name("grafana".to_string()).add_system(test_system.clone());
    println!("DEBUG {:?}", test_ns);
    let test_ns_create = match namespace::NamespaceStore::create(test_ns.clone(), vec!["uid".to_string(), "system { uid }".to_string()]) {
        Ok(v) => {
            println!("CREATE NS => {:?}", v);
            v
        },
        Err(e) => {
            println!("{:?}", e);
            None
        }
    };
    test_ns.uid = test_ns_create.ok_or(NamespaceError::Empty())?.uid;
    let test_scope = scope::Scope::new().name("admin".to_string()).scope_type(scope::ScopeType::group).add_namespace(test_ns.clone());
    let test_scope_create = match scope::ScopeStore::create(test_scope.clone(), vec!["uid".to_string(), "namespace { uid }".to_string()]) {
        Ok(v) => {
            println!("Create Scope => {:?}", v);
            v
        },
        Err(e) => {
            println!("{:?}", e);
            None
        }
    };
    let _ = match scope::ScopeStore::associate_entity(test_scope.clone().guid.as_ref().ok_or(ScopeError::Empty())?, test_e.clone(), vec!["uid".to_string(), "scope { uid }".to_string()]) {
        Ok(v) => {
            println!("Associate scope => {:?}", v);
        }
        Err(e) => {
            println!("{:?}", e);
        }
    };
    Ok(())
}
