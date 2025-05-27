
// Auto-generated reusable types for handler schemas
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Pet {
    pub age: i32,
    pub breed: String,
    pub id: i32,
    pub name: String,
    pub tags: Vec<serde_json::Value>,
    pub vaccinated: bool,
    }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AdminSettings {
    pub feature_flags: serde_json::Value,
    }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct UserList {
    pub users: Vec<User>,
    }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PostItemRequest {
    pub name: String,
    }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AdminSettingsResponse {
    pub feature_flags: serde_json::Value,
    }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AddPetRequest {
    pub name: String,
    }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct User {
    pub id: String,
    pub name: String,
    }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ListPetsResponse {
    pub items: Vec<Pet>,
    }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AddPetResponse {
    pub id: i32,
    pub status: String,
    }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Item {
    pub id: String,
    pub name: String,
    }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GetItemResponse {
    pub id: String,
    pub name: String,
    }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GetPetResponse {
    pub age: i32,
    pub breed: String,
    pub id: i32,
    pub name: String,
    pub tags: Vec<serde_json::Value>,
    pub vaccinated: bool,
    }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ListUsersResponse {
    pub users: Vec<User>,
    }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GetUserResponse {
    pub id: String,
    pub name: String,
    }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CreatePetRequest {
    pub name: String,
    }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PetCreationResponse {
    pub id: i32,
    pub status: String,
    }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Post {
    pub body: String,
    pub id: String,
    pub title: String,
    }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CreateItemRequest {
    pub name: String,
    }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ListUserPostsResponse {
    pub items: Vec<Post>,
    }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GetPostResponse {
    pub body: String,
    pub id: String,
    pub title: String,
    }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PostItemResponse {
    pub id: String,
    pub name: String,
    }
