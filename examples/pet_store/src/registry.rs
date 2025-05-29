// Auto-generated handler registry
use crate::controllers::*;
use crate::handlers::*;
use brrtrouter::dispatcher::Dispatcher;
use brrtrouter::spec::RouteMeta;
use brrtrouter::typed::spawn_typed;

pub unsafe fn register_all(dispatcher: &mut Dispatcher) {
    dispatcher.register_typed(
        "admin_settings",
        crate::controllers::admin_settings::AdminSettingsController,
    );
    dispatcher.register_typed("get_item", crate::controllers::get_item::GetItemController);
    dispatcher.register_typed(
        "post_item",
        crate::controllers::post_item::PostItemController,
    );
    dispatcher.register_typed(
        "list_pets",
        crate::controllers::list_pets::ListPetsController,
    );
    dispatcher.register_typed("add_pet", crate::controllers::add_pet::AddPetController);
    dispatcher.register_typed("get_pet", crate::controllers::get_pet::GetPetController);
    dispatcher.register_typed(
        "list_users",
        crate::controllers::list_users::ListUsersController,
    );
    dispatcher.register_typed("get_user", crate::controllers::get_user::GetUserController);
    dispatcher.register_typed(
        "list_user_posts",
        crate::controllers::list_user_posts::ListUserPostsController,
    );
    dispatcher.register_typed("get_post", crate::controllers::get_post::GetPostController);
}

/// Dynamically register handlers for the provided routes using their handler names.
pub unsafe fn register_from_spec(dispatcher: &mut Dispatcher, routes: &[RouteMeta]) {
    for route in routes {
        match route.handler_name.as_str() {
            "admin_settings" => {
                let tx = spawn_typed::<crate::handlers::admin_settings::Request, crate::handlers::admin_settings::Response, crate::controllers::admin_settings::AdminSettingsController>(crate::controllers::admin_settings::AdminSettingsController);
                dispatcher.add_route(route.clone(), tx);
            }
            "get_item" => {
                let tx = spawn_typed::<crate::handlers::get_item::Request, crate::handlers::get_item::Response, crate::controllers::get_item::GetItemController>(crate::controllers::get_item::GetItemController);
                dispatcher.add_route(route.clone(), tx);
            }
            "post_item" => {
                let tx = spawn_typed::<crate::handlers::post_item::Request, crate::handlers::post_item::Response, crate::controllers::post_item::PostItemController>(crate::controllers::post_item::PostItemController);
                dispatcher.add_route(route.clone(), tx);
            }
            "list_pets" => {
                let tx = spawn_typed::<crate::handlers::list_pets::Request, crate::handlers::list_pets::Response, crate::controllers::list_pets::ListPetsController>(crate::controllers::list_pets::ListPetsController);
                dispatcher.add_route(route.clone(), tx);
            }
            "add_pet" => {
                let tx = spawn_typed::<crate::handlers::add_pet::Request, crate::handlers::add_pet::Response, crate::controllers::add_pet::AddPetController>(crate::controllers::add_pet::AddPetController);
                dispatcher.add_route(route.clone(), tx);
            }
            "get_pet" => {
                let tx = spawn_typed::<crate::handlers::get_pet::Request, crate::handlers::get_pet::Response, crate::controllers::get_pet::GetPetController>(crate::controllers::get_pet::GetPetController);
                dispatcher.add_route(route.clone(), tx);
            }
            "list_users" => {
                let tx = spawn_typed::<crate::handlers::list_users::Request, crate::handlers::list_users::Response, crate::controllers::list_users::ListUsersController>(crate::controllers::list_users::ListUsersController);
                dispatcher.add_route(route.clone(), tx);
            }
            "get_user" => {
                let tx = spawn_typed::<crate::handlers::get_user::Request, crate::handlers::get_user::Response, crate::controllers::get_user::GetUserController>(crate::controllers::get_user::GetUserController);
                dispatcher.add_route(route.clone(), tx);
            }
            "list_user_posts" => {
                let tx = spawn_typed::<crate::handlers::list_user_posts::Request, crate::handlers::list_user_posts::Response, crate::controllers::list_user_posts::ListUserPostsController>(crate::controllers::list_user_posts::ListUserPostsController);
                dispatcher.add_route(route.clone(), tx);
            }
            "get_post" => {
                let tx = spawn_typed::<crate::handlers::get_post::Request, crate::handlers::get_post::Response, crate::controllers::get_post::GetPostController>(crate::controllers::get_post::GetPostController);
                dispatcher.add_route(route.clone(), tx);
            }
            _ => {}
        }
    }
}
