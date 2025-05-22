// Auto-generated handler registry

use crate::dispatcher::Dispatcher;
use crate::handlers::*;

pub unsafe fn register_all(dispatcher: &mut Dispatcher) {
    dispatcher.register_handler("admin_settings", |req| { admin_settings::handler(req.into()); });
    dispatcher.register_handler("get_item", |req| { get_item::handler(req.into()); });
    dispatcher.register_handler("post_item", |req| { post_item::handler(req.into()); });
    dispatcher.register_handler("list_pets", |req| { list_pets::handler(req.into()); });
    dispatcher.register_handler("add_pet", |req| { add_pet::handler(req.into()); });
    dispatcher.register_handler("get_pet", |req| { get_pet::handler(req.into()); });
    dispatcher.register_handler("list_users", |req| { list_users::handler(req.into()); });
    dispatcher.register_handler("get_user", |req| { get_user::handler(req.into()); });
    dispatcher.register_handler("list_user_posts", |req| { list_user_posts::handler(req.into()); });
    dispatcher.register_handler("get_post", |req| { get_post::handler(req.into()); });
    
}