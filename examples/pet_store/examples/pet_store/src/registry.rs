
// Auto-generated handler registry
use brrtrouter::dispatcher::Dispatcher;
use brrtrouter::spec::RouteMeta;
use brrtrouter::typed::spawn_typed;
// Note: avoid wildcard imports to reduce warnings

/// # Safety
/// This function spawns handler coroutines. Callers must ensure coroutine runtime is set up.
pub unsafe fn register_all(dispatcher: &mut Dispatcher) {
    dispatcher.register_typed(
        "admin_settings",
        crate::controllers::admin_settings::AdminSettingsController,
    );
    dispatcher.register_typed(
        "download_file",
        crate::controllers::download_file::DownloadFileController,
    );
    dispatcher.register_typed(
        "stream_events",
        crate::controllers::stream_events::StreamEventsController,
    );
    dispatcher.register_typed(
        "submit_form",
        crate::controllers::submit_form::SubmitFormController,
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
        "get_label",
        crate::controllers::get_label::GetLabelController,
    );
    dispatcher.register_typed(
        "get_matrix",
        crate::controllers::get_matrix::GetMatrixController,
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
        "search",
        crate::controllers::search::SearchController,
    );
    dispatcher.register_typed(
        "secure_endpoint",
        crate::controllers::secure_endpoint::SecureEndpointController,
    );
    dispatcher.register_typed(
        "upload_file",
        crate::controllers::upload_file::UploadFileController,
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
        "delete_user",
        crate::controllers::delete_user::DeleteUserController,
    );
    dispatcher.register_typed(
        "options_user",
        crate::controllers::options_user::OptionsUserController,
    );
    dispatcher.register_typed(
        "head_user",
        crate::controllers::head_user::HeadUserController,
    );
    dispatcher.register_typed(
        "list_user_posts",
        crate::controllers::list_user_posts::ListUserPostsController,
    );
    dispatcher.register_typed(
        "get_post",
        crate::controllers::get_post::GetPostController,
    );
    dispatcher.register_typed(
        "register_webhook",
        crate::controllers::register_webhook::RegisterWebhookController,
    );
    
}

/// Dynamically register handlers for the provided routes using their handler names.
/// # Safety
/// This function spawns handler coroutines. Callers must ensure coroutine runtime is set up.
/// 
/// **IMPORTANT**: This function will clear ALL existing handlers before registering new ones
/// to prevent memory leaks from accumulating coroutines.
pub unsafe fn register_from_spec(dispatcher: &mut Dispatcher, routes: &[RouteMeta]) {
    // Clear all existing handlers to prevent memory leaks
    // The old senders will be dropped, causing their coroutines to exit
    dispatcher.handlers.clear();
    
    for route in routes {
        match route.handler_name.as_str() {
            "admin_settings" => {
                let tx = spawn_typed(crate::controllers::admin_settings::AdminSettingsController);
                dispatcher.add_route(route.clone(), tx);
            }
            "download_file" => {
                let tx = spawn_typed(crate::controllers::download_file::DownloadFileController);
                dispatcher.add_route(route.clone(), tx);
            }
            "stream_events" => {
                let tx = spawn_typed(crate::controllers::stream_events::StreamEventsController);
                dispatcher.add_route(route.clone(), tx);
            }
            "submit_form" => {
                let tx = spawn_typed(crate::controllers::submit_form::SubmitFormController);
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
            "get_label" => {
                let tx = spawn_typed(crate::controllers::get_label::GetLabelController);
                dispatcher.add_route(route.clone(), tx);
            }
            "get_matrix" => {
                let tx = spawn_typed(crate::controllers::get_matrix::GetMatrixController);
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
            "search" => {
                let tx = spawn_typed(crate::controllers::search::SearchController);
                dispatcher.add_route(route.clone(), tx);
            }
            "secure_endpoint" => {
                let tx = spawn_typed(crate::controllers::secure_endpoint::SecureEndpointController);
                dispatcher.add_route(route.clone(), tx);
            }
            "upload_file" => {
                let tx = spawn_typed(crate::controllers::upload_file::UploadFileController);
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
            "delete_user" => {
                let tx = spawn_typed(crate::controllers::delete_user::DeleteUserController);
                dispatcher.add_route(route.clone(), tx);
            }
            "options_user" => {
                let tx = spawn_typed(crate::controllers::options_user::OptionsUserController);
                dispatcher.add_route(route.clone(), tx);
            }
            "head_user" => {
                let tx = spawn_typed(crate::controllers::head_user::HeadUserController);
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
            "register_webhook" => {
                let tx = spawn_typed(crate::controllers::register_webhook::RegisterWebhookController);
                dispatcher.add_route(route.clone(), tx);
            }
            
            _ => {}
        }
    }
}