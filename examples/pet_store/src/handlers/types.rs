// Auto-generated reusable types for handler schemas
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AddPetRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AddPetResponse {
    pub id: i32,

    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AdminSettings {
    pub feature_flags: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AdminSettingsResponse {
    pub feature_flags: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Cat {
    pub meow: bool,

    pub r#type: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CreateItemRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CreatePetRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Dog {
    pub bark: bool,

    pub r#type: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct DownloadFileResponse {
    pub id: String,

    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GetItemResponse {
    pub id: String,

    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GetLabelResponse {
    pub color: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GetMatrixResponse {
    pub coords: Vec<i32>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GetPetResponse {
    pub age: i32,

    pub breed: String,

    pub id: i32,

    pub name: String,

    pub tags: Vec<String>,

    pub vaccinated: bool,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GetPostResponse {
    pub author_id: String,

    pub body: String,

    pub id: String,

    pub title: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GetUserResponse {
    pub email: String,

    pub id: String,

    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct HeadUserResponse {
    pub exists: bool,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Item {
    pub id: String,

    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ListPetsResponse {
    pub items: Vec<Pet>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ListUserPostsResponse {
    pub items: Vec<Post>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ListUsersResponse {
    pub users: Vec<User>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct OptionsUserResponse {
    pub allow: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Pet {
    pub age: i32,

    pub breed: String,

    pub id: i32,

    pub name: String,

    pub tags: Vec<String>,

    pub vaccinated: bool,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PetCreationResponse {
    pub id: i32,

    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Post {
    pub author_id: String,

    pub body: String,

    pub id: String,

    pub title: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PostItemRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PostItemResponse {
    pub id: String,

    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ProblemDetails {
    pub detail: String,

    pub errors: Vec<String>,

    pub instance: String,

    pub status: i32,

    pub title: String,

    pub r#type: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct RegisterWebhookRequest {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct RegisterWebhookResponse {
    pub subscription_id: String,

    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SearchResponse {
    pub results: Vec<Item>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SecureEndpointResponse {
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SubmitFormResponse {
    pub ok: bool,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct UploadFileResponse {
    pub location: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct User {
    pub email: String,

    pub id: String,

    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct UserList {
    pub users: Vec<User>,
}
