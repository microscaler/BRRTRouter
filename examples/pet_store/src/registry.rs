
// Auto-generated handler registry
use brrtrouter::dispatcher::Dispatcher;
use brrtrouter::spec::RouteMeta;
use brrtrouter::typed::spawn_typed;
use crate::controllers::*;
use crate::handlers::*;

pub unsafe fn register_all(dispatcher: &mut Dispatcher) {
    dispatcher.register_typed(
        "admin_settings",
        crate::controllers::admin_settings::AdminSettingsController,
    );
    dispatcher.register_typed(
        "get_item",
        crate::controllers::get_item::GetItemController,
    );
    dispatcher.register_typed(
        "post_item",
        crate::controllers::post_item::PostItemController,
    );
    dispatcher.register_typed(
        "list_pets",
        crate::controllers::list_pets::ListPetsController,
    );
    dispatcher.register_typed(
        "add_pet",
        crate::controllers::add_pet::AddPetController,
    );
    dispatcher.register_typed(
        "get_pet",
        crate::controllers::get_pet::GetPetController,
    );
    dispatcher.register_typed(
        "list_users",
        crate::controllers::list_users::ListUsersController,
    );
    dispatcher.register_typed(
        "get_user",
        crate::controllers::get_user::GetUserController,
    );
    dispatcher.register_typed(
        "list_user_posts",
        crate::controllers::list_user_posts::ListUserPostsController,
    );
    dispatcher.register_typed(
        "get_post",
        crate::controllers::get_post::GetPostController,
    );
    
}

/// Dynamically register handlers for the provided routes using their handler names.
pub unsafe fn register_from_spec(dispatcher: &mut Dispatcher, routes: &[RouteMeta]) {
    for route in routes {
        match route.handler_name.as_str() {
            "admin_settings" => {
                let tx = spawn_typed(crate::controllers::admin_settings::AdminSettingsController);
                dispatcher.add_route(route.clone(), tx);
            }
            "get_item" => {
                let tx = spawn_typed(crate::controllers::get_item::GetItemController);
                dispatcher.add_route(route.clone(), tx);
            }
            "post_item" => {
                let tx = spawn_typed(crate::controllers::post_item::PostItemController);
                dispatcher.add_route(route.clone(), tx);
            }
            "list_pets" => {
                let tx = spawn_typed(crate::controllers::list_pets::ListPetsController);
                dispatcher.add_route(route.clone(), tx);
            }
            "add_pet" => {
                let tx = spawn_typed(crate::controllers::add_pet::AddPetController);
                dispatcher.add_route(route.clone(), tx);
            }
            "get_pet" => {
                let tx = spawn_typed(crate::controllers::get_pet::GetPetController);
                dispatcher.add_route(route.clone(), tx);
            }
            "list_users" => {
                let tx = spawn_typed(crate::controllers::list_users::ListUsersController);
                dispatcher.add_route(route.clone(), tx);
            }
            "get_user" => {
                let tx = spawn_typed(crate::controllers::get_user::GetUserController);
                dispatcher.add_route(route.clone(), tx);
            }
            "list_user_posts" => {
                let tx = spawn_typed(crate::controllers::list_user_posts::ListUserPostsController);
                dispatcher.add_route(route.clone(), tx);
            }
            "get_post" => {
                let tx = spawn_typed(crate::controllers::get_post::GetPostController);
                dispatcher.add_route(route.clone(), tx);
            }
            
            _ => {}
        }
    }
}
