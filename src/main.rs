extern crate dgraph;
extern crate once_cell;
extern crate nanoid;
extern crate handlebars;
extern crate serde_derive;
extern crate actix_web;
extern crate bytes;
extern crate json;
extern crate rand;
extern crate argon2;
extern crate reqwest;
extern crate csrf;
extern crate data_encoding;


use crate::identifier::{IdentifierError, IdentifierType, Identifier};
use crate::entity::EntityError;
use crate::system::{SystemError, System};
use crate::authenticator::{AuthenticatorError, Authenticator};
use crate::namespace::NamespaceError;
use crate::scope::ScopeError;
use actix_web::{
    error, middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer,
};
use bytes::{Bytes, BytesMut};
use futures::StreamExt;
use json::JsonValue;
use serde::{Deserialize, Serialize};
use rand::rngs::OsRng;
use rand::RngCore;
use crate::hydra::{Hydra, HydraLoginResponse, HydraAcceptLoginRequest};
use serde_json::json;
use actix_http::cookie::Cookie;
use csrf::{AesGcmCsrfProtection, CsrfProtection};
use data_encoding::BASE64;
use handlebars::Handlebars;
use once_cell::unsync::OnceCell;
use reqwest::header::HeaderValue;

mod system;
mod authenticator;
mod identifier;
mod entity;
mod db;
mod scope;
mod namespace;
mod hydra;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct LoginReq {
    identifier: String,
    authenticator: String,
    authenticator_type: authenticator::AuthenticatorType,
    system: String,
    _csrf: String,
    challenge: String
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct HydraLogin {
    challenge: String
}

#[derive(Clone)]
struct AppData<'a> {
    csrf_generator: web::Data<std::sync::Mutex<csrf::AesGcmCsrfProtection>>,
    hb: web::Data<Handlebars<'a>>
}

async fn login_form(query: web::Query<HydraLogin>, data: web::Data<AppData<'_>>) -> HttpResponse {
    let generator = data.csrf_generator.lock().unwrap();
    let challenge = query.clone().challenge;
    let (token, cookie) = generator.generate_token_pair(None, 3600).unwrap();
    let token_str = token.b64_string();
    let cookie_str = cookie.b64_string();

    drop(generator);

    let resp: HydraLoginResponse = serde_json::from_str(Hydra::get_login_request(challenge.clone()).await.unwrap().as_str()).unwrap();

    if resp.skip {
        let accept_login = HydraAcceptLoginRequest {
            subject: resp.subject.clone(),
            remember: false,
            remember_for: 3600
        };

        let resp = Hydra::accept_login_request(challenge.clone(), serde_json::to_string(&accept_login).unwrap()).await.unwrap();

        println!("{:?}", resp);

        return HttpResponse::Ok().finish()

        // HttpResponse::Found().header(actix_web::http::header::LOCATION, resp)
    }

    let tmpl_data = json!({
        "challenge": challenge.clone(),
        "csrf_token": token_str
    });


    let body = data.hb.render("login", &tmpl_data).unwrap();

    let csrf_cookie = Cookie::new("_csrf", cookie_str.clone());

    HttpResponse::Ok().header(actix_web::http::header::SET_COOKIE, csrf_cookie.to_string()).body(body)
}

async fn login(item: web::Form<LoginReq>) -> HttpResponse {
    println!("REQUEST POST: {:?}", item);
    let challenge = item.challenge.clone();

    let result = authenticator::AuthenticatorStore::login(
        Authenticator::new().authenticator_type(item.authenticator_type.clone()).value(item.authenticator.clone()),
        Identifier::new().identifier_type(IdentifierType::email).value(item.identifier.clone()),
        System::new().guid(item.system.clone())
    );

    if result.is_err() || !result.ok().unwrap() {
        return HttpResponse::Found().header(actix_web::http::header::LOCATION, format!("/login?challenge={}", challenge.clone())).finish()
    }

    println!("Received challenge {} and creds are good", challenge);
    // let generator = data.lock().unwrap();
    // let (token, cookie) = generator.generate_token_pair(None, 3600).unwrap();
    // let token_str = token.b64_string();
    // let cookie_str = cookie.b64_string();
    // let challenge = query.clone().challenge;
    //
    // println!("{}", challenge.clone());
    //
    // let resp: HydraLoginResponse = serde_json::from_str(Hydra::get_login_request(challenge).await.unwrap().as_str()).unwrap();
    //
    // println!("{:?}", resp);
    //
    // let inner = item.into_inner();
    //
    let accept_login = HydraAcceptLoginRequest {
        subject: item.identifier.clone(),
        remember: false,
        remember_for: 3600
    };

    let resp = Hydra::accept_login_request(item.challenge.clone(), serde_json::to_string(&accept_login).unwrap()).await.unwrap();
    //
    // println!("{:?}", resp);
    //
    // let result = authenticator::AuthenticatorStore::login(
    //     Authenticator::new().authenticator_type(inner.clone().authenticator_type).value(inner.clone().authenticator),
    //     Identifier::new().identifier_type(IdentifierType::email).value(inner.clone().identifier),
    //     System::new().guid(inner.clone().system)
    // );
    //
    // if result.is_err() || !result.ok().unwrap() {
    //     return HttpResponse::Unauthorized().finish()
    // }
    //
    // let csrf_cookie = Cookie::new("_csrf", cookie_str.clone());
    // let token_cookie = Cookie::new("_csrf_token", token_str.clone());
    //
    // drop(generator);
    //
    let resp_json: serde_json::Value = serde_json::from_str(resp.clone().as_str()).unwrap();
    //
    // let client: reqwest::Client = reqwest::Client::new();
    //
    // let redirect_that_bitch = client.get(resp_json["redirect_to"].as_str().unwrap()).header("csrf", cookie_str).send().await.unwrap();
    //
    // println!("{:?}", redirect_that_bitch);

    return HttpResponse::Found().header(actix_web::http::header::LOCATION, resp_json["redirect_to"].as_str().unwrap()).finish()
}


#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // db::drop_all();
    // let schema = r#"
    //     display_name: string @index(trigram) .
	// 	guid: string @index(exact) .
	// 	sid: string @index(exact) .
	// 	value: string @index(trigram, exact) .
	// 	identifier_type: string @index(exact) .
	// 	authenticator_type: string @index(exact) .
	// 	identifier: [uid] @reverse .
	// 	entity: [uid] @reverse .
	// 	authenticator: [uid] @reverse .
	// 	name: string @index(trigram, exact) .
	// 	system: [uid] @reverse .
	// 	scope_type: string @index(exact) .
	// 	scope: [uid] @reverse .
	// 	namespace: [uid] @reverse .
    //
	// 	type Entity {
	// 		guid
	// 		sid
	// 		display_name
	// 		identifier
	// 		authenticator
	// 		system
	// 		scope
	// 	}
    //
	// 	type Identifier {
	// 	    entity
	// 	    authenticator
	// 		identifier_type
	// 		value
	// 	}
    //
	// 	type Authenticator {
	// 	    entity
	// 	    identifier
	// 	    authenticator_type
	// 	    value
	// 	}
    //
	// 	type System {
	// 	    guid
	// 	    name
	// 	    entity
	// 	    authenticator
	// 	    namespace
	// 	}
    //
	// 	type Namespace {
	// 	    guid
	// 	    name
	// 	    system
	// 	    scope
	// 	}
    //
	// 	type Scope {
	// 	    guid
	// 	    name
	// 	    namespace
	// 	    scope_type
	// 	    entity
	// 	}
    // "#;
    // db::migrate_schema(schema);
    //
    // let mut s = system::System::new().name("Enterprise".to_string());
    // let s_res = system::SystemStore::create(s.clone(), vec!["uid".to_string()]).unwrap();
    // s.uid = Some(s_res.unwrap().uid.unwrap());
    // println!("SYS GUID {}", s.clone().guid.unwrap());
    //
    // let mut e = entity::Entity::new().display_name("Test Account".to_string()).sid("testacc".to_string());
    // let e_res = entity::EntityStore::create(e.clone(), vec!["uid".to_string()]).unwrap();
    // e.uid = Some(e_res.unwrap().uid.unwrap());
    //
    // let mut i = identifier::Identifier::new().identifier_type(identifier::IdentifierType::email).value("test@test.com".to_string()).add_entity(e.clone());
    // let i_res = identifier::IdentifierStore::create(i.clone(), vec!["uid".to_string()]).unwrap();
    // i.uid = Some(i_res.unwrap().uid.unwrap());
    //
    // let random_u64 = OsRng.next_u64();
    //
    // let salt = format!("{:X}", random_u64);
    //
    // let config = argon2::Config::default();
    // let start = std::time::Instant::now();
    // let hash = argon2::hash_encoded("password123".as_bytes(), salt.as_bytes(), &config).unwrap();
    // println!("Took {}ms to hash", start.elapsed().as_millis());
    // let mut a = authenticator::Authenticator::new().authenticator_type(authenticator::AuthenticatorType::email_password).value(hash.to_string()).add_system(s.clone()).add_identifier(i.clone()).add_entity(e.clone());
    // let a_res = authenticator::AuthenticatorStore::create(a.clone(), vec!["uid".to_string()]).unwrap();

    let generator = web::Data::new(std::sync::Mutex::new((AesGcmCsrfProtection::from_key(*b"01234567012345670123456701234567"))));

    let mut handlebars = Handlebars::new();
    handlebars
        .register_templates_directory(".html", "./static")
        .unwrap();
    let handlebars_ref = web::Data::new(handlebars);

    let app_data = AppData {
        hb: handlebars_ref.clone(),
        csrf_generator: generator.clone()
    };

    let app_data_ref = web::Data::new(app_data);

    HttpServer::new(move || {
        App::new()
            .app_data(app_data_ref.clone())
            .wrap(middleware::Logger::default())
            .service(
                web::resource("/login")
                    .route(web::post().to(login))
                    .route(web::get().to(login_form)))
    })
        .bind("localhost:8087")?
        .run()
        .await
}
