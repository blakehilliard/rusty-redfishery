use axum::async_trait;
use redfish_axum::{Error, Node, Tree};
use redfish_data::{
    get_uri_id, AllowedMethods, CollectionType, ResourceSchemaVersion, ResourceType,
};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

pub struct Collection {
    uri: String,
    resource_type: CollectionType,
    name: String,
    pub members: Vec<String>,
    // if user should not be able to POST to collection, this should be None
    // else, it should be a function that returns new Resource generated from Request
    // that function should *not* add the resource to the collection's members vector.
    post: Option<fn(&Collection, Map<String, Value>) -> Result<Resource, Error>>,
}

impl Collection {
    pub fn new(
        uri: &str,
        schema_name: String,
        name: String,
        members: Vec<String>,
        post: Option<fn(&Collection, Map<String, Value>) -> Result<Resource, Error>>,
    ) -> Self {
        Self {
            uri: String::from(uri),
            resource_type: CollectionType::new_dmtf_v1(schema_name),
            name,
            members,
            post,
        }
    }
}

impl Node for Collection {
    fn get_uri(&self) -> &str {
        self.uri.as_str()
    }

    fn get_body(&self) -> Value {
        let mut member_list = Vec::new();
        for member in self.members.iter() {
            let mut member_obj = HashMap::new();
            member_obj.insert(String::from("@odata.id"), member);
            member_list.push(member_obj);
        }
        json!({
            "@odata.id": self.uri,
            "@odata.etag": "\"HARDCODED_ETAG\"",
            "@odata.type": format!("#{}.{}", self.resource_type.name, self.resource_type.name),
            "Name": self.name,
            "Members": member_list,
            "Members@odata.count": self.members.len(),
        })
    }

    fn get_allowed_methods(&self) -> AllowedMethods {
        AllowedMethods {
            delete: false,
            get: true,
            patch: false,
            post: self.post.is_some(),
        }
    }

    fn described_by(&self) -> Option<&str> {
        Some(self.resource_type.described_by.as_str())
    }
}

pub struct Resource {
    uri: String, //TODO: Enforce things here? Does DMTF recommend trailing slash or no?
    resource_type: ResourceType,
    pub body: Map<String, Value>,
    collection: Option<String>,
    // if user should not be able to PATCH this resource, this should be None
    // else, it should be a function that applies the patch.
    patch: Option<fn(&mut Resource, Value) -> Result<(), Error>>,
    // if use should not be able to DELETE this resource, this should be None.
    // else, it should be a function that performs any extra logic associated with deleting the resource.
    delete: Option<fn(&Resource) -> Result<(), Error>>,
}

impl Resource {
    pub fn new(
        uri: &str,
        schema_name: String,
        schema_version: ResourceSchemaVersion,
        term_name: String,
        name: String,
        delete: Option<fn(&Resource) -> Result<(), Error>>,
        patch: Option<fn(&mut Resource, Value) -> Result<(), Error>>,
        collection: Option<String>,
        rest: Value,
    ) -> Self {
        let mut body = rest.as_object().unwrap().clone();
        body.insert(String::from("@odata.id"), json!(uri));
        body.insert(String::from("@odata.etag"), json!("\"HARDCODED_ETAG\""));
        body.insert(
            String::from("@odata.type"),
            json!(format!(
                "#{}.{}.{}",
                schema_name,
                schema_version.to_string(),
                term_name
            )),
        );
        let id = get_uri_id(uri);
        body.insert(String::from("Id"), json!(id));
        body.insert(String::from("Name"), json!(name));
        let resource_type = ResourceType::new_dmtf(schema_name, schema_version);
        Self {
            uri: String::from(uri),
            resource_type,
            body,
            delete,
            patch,
            collection,
        }
    }
}

impl Node for Resource {
    fn get_uri(&self) -> &str {
        self.uri.as_str()
    }

    fn get_body(&self) -> Value {
        Value::Object(self.body.clone())
    }

    fn get_allowed_methods(&self) -> AllowedMethods {
        AllowedMethods {
            delete: self.delete.is_some(),
            get: true,
            patch: self.patch.is_some(),
            post: false,
        }
    }

    fn described_by(&self) -> Option<&str> {
        Some(self.resource_type.described_by.as_str())
    }
}

pub struct MockTree {
    resources: HashMap<String, Resource>,
    collections: HashMap<String, Collection>,
    collection_types: Vec<CollectionType>,
    resource_types: Vec<ResourceType>,
}

impl MockTree {
    pub fn new() -> Self {
        Self {
            resources: HashMap::new(),
            collections: HashMap::new(),
            collection_types: Vec::new(),
            resource_types: Vec::new(),
        }
    }

    pub fn add_resource(&mut self, resource: Resource) {
        let resource_type = resource.resource_type.clone();
        self.resources.insert(resource.uri.clone(), resource);
        if !self.resource_types.contains(&resource_type) {
            self.resource_types.push(resource_type);
        }
    }

    pub fn add_collection(&mut self, collection: Collection) {
        let collection_type = collection.resource_type.clone();
        self.collections.insert(collection.uri.clone(), collection);
        if !self.collection_types.contains(&collection_type) {
            self.collection_types.push(collection_type);
        }
    }
}

#[async_trait]
impl Tree for MockTree {
    async fn get(&self, uri: &str, username: Option<&str>) -> Result<&dyn Node, Error> {
        if uri != "/redfish/v1" && username.is_none() {
            return Err(Error::Unauthorized);
        }
        if let Some(resource) = self.resources.get(uri) {
            return Ok(resource);
        }
        if let Some(collection) = self.collections.get(uri) {
            return Ok(collection);
        }
        Err(Error::NotFound)
    }

    async fn create(
        &mut self,
        uri: &str,
        req: Map<String, Value>,
        username: Option<&str>,
    ) -> Result<&dyn Node, Error> {
        if uri != "/redfish/v1/SessionService/Sessions" && username.is_none() {
            return Err(Error::Unauthorized);
        }
        match self.collections.get_mut(uri) {
            None => match self.resources.get(uri) {
                Some(resource) => Err(Error::MethodNotAllowed(resource.get_allowed_methods())),
                None => Err(Error::NotFound),
            },
            Some(collection) => match collection.post {
                None => Err(Error::MethodNotAllowed(collection.get_allowed_methods())),
                Some(post) => {
                    let member = post(collection, req)?;
                    let member_uri = member.uri.clone();
                    self.resources.insert(member.uri.clone(), member);
                    // Update members of collection.
                    collection.members.push(member_uri.clone());
                    // Return new resource.
                    Ok(self.resources.get(&member_uri).unwrap())
                }
            },
        }
    }

    async fn delete(&mut self, uri: &str, username: Option<&str>) -> Result<(), Error> {
        if username.is_none() {
            return Err(Error::Unauthorized);
        }
        match self.resources.get(uri) {
            None => match self.collections.get(uri) {
                Some(collection) => Err(Error::MethodNotAllowed(collection.get_allowed_methods())),
                None => Err(Error::NotFound),
            },
            Some(resource) => match resource.delete {
                None => Err(Error::MethodNotAllowed(resource.get_allowed_methods())),
                Some(delete) => {
                    delete(resource)?;
                    if let Some(collection_uri) = &resource.collection {
                        if let Some(collection) = self.collections.get_mut(collection_uri) {
                            if let Some(member_index) =
                                collection.members.iter().position(|x| x == uri)
                            {
                                collection.members.remove(member_index);
                            }
                        }
                    }
                    self.resources.remove(uri);
                    Ok(())
                }
            },
        }
    }

    async fn patch(
        &mut self,
        uri: &str,
        req: Value,
        username: Option<&str>,
    ) -> Result<&dyn Node, Error> {
        if username.is_none() {
            return Err(Error::Unauthorized);
        }
        match self.resources.get_mut(uri) {
            None => match self.collections.get(uri) {
                Some(collection) => Err(Error::MethodNotAllowed(collection.get_allowed_methods())),
                None => Err(Error::NotFound),
            },
            Some(resource) => match resource.patch {
                None => Err(Error::MethodNotAllowed(resource.get_allowed_methods())),
                Some(patch) => {
                    patch(resource, req)?;
                    Ok(resource)
                }
            },
        }
    }

    fn get_collection_types(&self) -> &[CollectionType] {
        &self.collection_types
    }

    fn get_resource_types(&self) -> &[ResourceType] {
        &self.resource_types
    }
}
