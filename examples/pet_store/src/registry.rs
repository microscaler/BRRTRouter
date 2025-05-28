
// Auto-generated handler registry

use brrtrouter::dispatcher::Dispatcher;
use crate::handlers::*;

use crate::handlers::admin_settings::IntoTypedRequest;
use crate::handlers::admin_settings::admin_settings::Request;
use crate::handlers::get_item::IntoTypedRequest;
use crate::handlers::get_item::get_item::Request;
use crate::handlers::post_item::IntoTypedRequest;
use crate::handlers::post_item::post_item::Request;
use crate::handlers::list_pets::IntoTypedRequest;
use crate::handlers::list_pets::list_pets::Request;
use crate::handlers::add_pet::IntoTypedRequest;
use crate::handlers::add_pet::add_pet::Request;
use crate::handlers::get_pet::IntoTypedRequest;
use crate::handlers::get_pet::get_pet::Request;
use crate::handlers::list_users::IntoTypedRequest;
use crate::handlers::list_users::list_users::Request;
use crate::handlers::get_user::IntoTypedRequest;
use crate::handlers::get_user::get_user::Request;
use crate::handlers::list_user_posts::IntoTypedRequest;
use crate::handlers::list_user_posts::list_user_posts::Request;
use crate::handlers::get_post::IntoTypedRequest;
use crate::handlers::get_post::get_post::Request;


pub unsafe fn register_all(dispatcher: &mut Dispatcher) {
    dispatcher.register_handler("admin_settings", |req| {
        admin_settings::handler(
            brrtrouter::typed::TypedHandlerRequest::<admin_settings::Request>::from(
                req.into_typed_request(),
            ),
        );
    });
    dispatcher.register_handler("get_item", |req| {
        get_item::handler(
            brrtrouter::typed::TypedHandlerRequest::<get_item::Request>::from(
                req.into_typed_request(),
            ),
        );
    });
    dispatcher.register_handler("post_item", |req| {
        post_item::handler(
            brrtrouter::typed::TypedHandlerRequest::<post_item::Request>::from(
                req.into_typed_request(),
            ),
        );
    });
    dispatcher.register_handler("list_pets", |req| {
        list_pets::handler(
            brrtrouter::typed::TypedHandlerRequest::<list_pets::Request>::from(
                req.into_typed_request(),
            ),
        );
    });
    dispatcher.register_handler("add_pet", |req| {
        add_pet::handler(
            brrtrouter::typed::TypedHandlerRequest::<add_pet::Request>::from(
                req.into_typed_request(),
            ),
        );
    });
    dispatcher.register_handler("get_pet", |req| {
        get_pet::handler(
            brrtrouter::typed::TypedHandlerRequest::<get_pet::Request>::from(
                req.into_typed_request(),
            ),
        );
    });
    dispatcher.register_handler("list_users", |req| {
        list_users::handler(
            brrtrouter::typed::TypedHandlerRequest::<list_users::Request>::from(
                req.into_typed_request(),
            ),
        );
    });
    dispatcher.register_handler("get_user", |req| {
        get_user::handler(
            brrtrouter::typed::TypedHandlerRequest::<get_user::Request>::from(
                req.into_typed_request(),
            ),
        );
    });
    dispatcher.register_handler("list_user_posts", |req| {
        list_user_posts::handler(
            brrtrouter::typed::TypedHandlerRequest::<list_user_posts::Request>::from(
                req.into_typed_request(),
            ),
        );
    });
    dispatcher.register_handler("get_post", |req| {
        get_post::handler(
            brrtrouter::typed::TypedHandlerRequest::<get_post::Request>::from(
                req.into_typed_request(),
            ),
        );
    });
    
}