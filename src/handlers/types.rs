// Auto-generated reusable types for handler schemas
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateItemRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePetRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Pet {
    pub age: i32,
    pub breed: String,
    pub id: i32,
    pub name: String,
    pub tags: Vec<serde_json::Value>,
    pub vaccinated: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PetCreationResponse {
    pub id: i32,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListUserPostsResponse {
    pub items: Vec<Post>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostItemRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AdminSettings {
    pub feature_flags: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetItemResponse {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListUsersResponse {
    pub users: Vec<User>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetPetResponse {
    pub age: i32,
    pub breed: String,
    pub id: i32,
    pub name: String,
    pub tags: Vec<serde_json::Value>,
    pub vaccinated: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetPostResponse {
    pub body: String,
    pub id: String,
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetUserResponse {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserList {
    pub users: Vec<User>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Post {
    pub body: String,
    pub id: String,
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AdminSettingsResponse {
    pub feature_flags: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddPetRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListPetsResponse {
    pub items: Vec<Pet>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostItemResponse {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddPetResponse {
    pub id: i32,
    pub status: String,
}
