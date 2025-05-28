
// Auto-generated handler registry
use brrtrouter::{dispatcher::Dispatcher, spec::{ParameterMeta, ParameterLocation}};
use crate::controllers::*;
use crate::handlers::*;

pub unsafe fn register_all(dispatcher: &mut Dispatcher) {
    dispatcher.register_typed(
        "admin_settings",
        crate::controllers::admin_settings::AdminSettingsController,
        vec![
            
        ],
    );
    dispatcher.register_typed(
        "get_item",
        crate::controllers::get_item::GetItemController,
        vec![
            ParameterMeta {
                name: "id".to_string(),
                location: ParameterLocation::Path,
                required: true,
                schema: {
                    
                    Some(serde_json::json!({"format":"uuid","type":"string"}))
                    
                },
            },
            
        ],
    );
    dispatcher.register_typed(
        "post_item",
        crate::controllers::post_item::PostItemController,
        vec![
            ParameterMeta {
                name: "id".to_string(),
                location: ParameterLocation::Path,
                required: true,
                schema: {
                    
                    Some(serde_json::json!({"format":"uuid","type":"string"}))
                    
                },
            },
            
        ],
    );
    dispatcher.register_typed(
        "list_pets",
        crate::controllers::list_pets::ListPetsController,
        vec![
            
        ],
    );
    dispatcher.register_typed(
        "add_pet",
        crate::controllers::add_pet::AddPetController,
        vec![
            
        ],
    );
    dispatcher.register_typed(
        "get_pet",
        crate::controllers::get_pet::GetPetController,
        vec![
            ParameterMeta {
                name: "id".to_string(),
                location: ParameterLocation::Path,
                required: true,
                schema: {
                    
                    Some(serde_json::json!({"type":"string"}))
                    
                },
            },
            
        ],
    );
    dispatcher.register_typed(
        "list_users",
        crate::controllers::list_users::ListUsersController,
        vec![
            
        ],
    );
    dispatcher.register_typed(
        "get_user",
        crate::controllers::get_user::GetUserController,
        vec![
            ParameterMeta {
                name: "user_id".to_string(),
                location: ParameterLocation::Path,
                required: true,
                schema: {
                    
                    Some(serde_json::json!({"type":"string"}))
                    
                },
            },
            
        ],
    );
    dispatcher.register_typed(
        "list_user_posts",
        crate::controllers::list_user_posts::ListUserPostsController,
        vec![
            ParameterMeta {
                name: "user_id".to_string(),
                location: ParameterLocation::Path,
                required: true,
                schema: {
                    
                    Some(serde_json::json!({"type":"string"}))
                    
                },
            },
            
        ],
    );
    dispatcher.register_typed(
        "get_post",
        crate::controllers::get_post::GetPostController,
        vec![
            ParameterMeta {
                name: "user_id".to_string(),
                location: ParameterLocation::Path,
                required: true,
                schema: {
                    
                    Some(serde_json::json!({"type":"string"}))
                    
                },
            },
            ParameterMeta {
                name: "post_id".to_string(),
                location: ParameterLocation::Path,
                required: true,
                schema: {
                    
                    Some(serde_json::json!({"type":"string"}))
                    
                },
            },
            
        ],
    );
    
}