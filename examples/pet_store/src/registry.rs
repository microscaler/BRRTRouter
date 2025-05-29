// Auto-generated handler registry
use crate::controllers::*;
use crate::handlers::*;
use brrtrouter::dispatcher::Dispatcher;

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
