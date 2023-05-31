use std::collections::HashMap;
use serde_json::{Value, json, Map};
use redfish_axum::{
    RedfishNode,
    RedfishTree, RedfishErr,
};
use redfish_data::{
    RedfishCollectionType,
    RedfishResourceType,
    RedfishSchemaVersion,
    RedfishResourceSchemaVersion,
    get_uri_id, AllowedMethods,
};

pub struct RedfishCollection {
    uri: String,
    resource_type: RedfishCollectionType,
    name: String,
    pub members: Vec<String>,
    // if user should not be able to POST to collection, this should be None
    // else, it should be a function that returns new RedfishResource generated from Request
    // that function should *not* add the resource to the collection's members vector.
    post: Option<fn(&RedfishCollection, serde_json::Value) -> Result<RedfishResource, RedfishErr>>,
}

impl RedfishCollection {
    pub fn new(
        uri: &str,
        schema_name: String,
        name: String,
        members: Vec<String>,
        post: Option<fn(&RedfishCollection, serde_json::Value) -> Result<RedfishResource, RedfishErr>>,
    ) -> Self {
        Self {
            uri: String::from(uri),
            resource_type: RedfishCollectionType::new_dmtf_v1(schema_name),
            name,
            members,
            post,
        }
    }
}

impl RedfishNode for RedfishCollection {
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
            "@odata.etag": "\"FIXME\"",
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

pub struct RedfishResource {
    uri: String, //TODO: Enforce things here? Does DMTF recommend trailing slash or no?
    resource_type: RedfishResourceType,
    pub body: Map<String, Value>,
    collection: Option<String>,
    // if user should not be able to PATCH this resource, this should be None
    // else, it should be a function that applies the patch.
    patch: Option<fn(&mut RedfishResource, serde_json::Value) -> Result<(), RedfishErr>>,
    // if use should not be able to DELETE this resource, this should be None.
    // else, it should be a function that performs any extra logic associated with deleting the resource.
    delete: Option<fn(&RedfishResource) -> Result<(), RedfishErr>>,
}

impl RedfishResource {
    pub fn new(
        uri: &str,
        schema_name: String,
        schema_version: RedfishResourceSchemaVersion,
        term_name: String,
        name: String,
        delete: Option<fn(&RedfishResource) -> Result<(), RedfishErr>>,
        patch: Option<fn(&mut RedfishResource, serde_json::Value) -> Result<(), RedfishErr>>,
        collection: Option<String>,
        rest: Value,
    ) -> Self {
        let mut body = rest.as_object().unwrap().clone();
        body.insert(String::from("@odata.id"), json!(uri));
        body.insert(String::from("@odata.etag"), json!("\"FIXME\""));
        body.insert(String::from("@odata.type"), json!(format!("#{}.{}.{}", schema_name, schema_version.to_str(), term_name)));
        let id = get_uri_id(uri);
        body.insert(String::from("Id"), json!(id));
        body.insert(String::from("Name"), json!(name));
        let resource_type = RedfishResourceType::new_dmtf(schema_name, schema_version);
        Self {
            uri: String::from(uri), resource_type, body, delete, patch, collection,
        }
    }
}

impl RedfishNode for RedfishResource {
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
    resources: HashMap<String, RedfishResource>,
    collections: HashMap<String, RedfishCollection>,
    collection_types: Vec<RedfishCollectionType>,
    resource_types: Vec<RedfishResourceType>,
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

    pub fn add_resource(&mut self, resource: RedfishResource) {
        let resource_type = resource.resource_type.clone();
        self.resources.insert(resource.uri.clone(), resource);
        if ! self.resource_types.contains(&resource_type) {
            self.resource_types.push(resource_type);
        }
    }

    pub fn add_collection(&mut self, collection: RedfishCollection) {
        let collection_type = collection.resource_type.clone();
        self.collections.insert(collection.uri.clone(), collection);
        if ! self.collection_types.contains(&collection_type) {
            self.collection_types.push(collection_type);
        }
    }
}

impl RedfishTree for MockTree {
    fn get(&self, uri: &str, username: Option<&str>) -> Result<&dyn RedfishNode, RedfishErr> {
        if uri != "/redfish/v1" && username.is_none() {
            return Err(RedfishErr::Unauthorized);
        }
        if let Some(resource) = self.resources.get(uri) {
            return Ok(resource);
        }
        if let Some(collection) = self.collections.get(uri) {
            return Ok(collection);
        }
        Err(RedfishErr::NotFound)
    }

    fn create(&mut self, uri: &str, req: serde_json::Value, username: Option<&str>) -> Result<&dyn RedfishNode, RedfishErr> {
        if uri != "/redfish/v1/SessionService/Sessions" && username.is_none() {
            return Err(RedfishErr::Unauthorized);
        }
        match self.collections.get_mut(uri) {
            None => match self.resources.get(uri) {
                Some(resource) => Err(RedfishErr::MethodNotAllowed(resource.get_allowed_methods())),
                None => Err(RedfishErr::NotFound),
            },
            Some(collection) => match collection.post {
                None => Err(RedfishErr::MethodNotAllowed(collection.get_allowed_methods())),
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

    fn delete(&mut self, uri: &str, username: Option<&str>) -> Result<(), RedfishErr> {
        if username.is_none() {
            return Err(RedfishErr::Unauthorized);
        }
        match self.resources.get(uri) {
            None => match self.collections.get(uri) {
                Some(collection) => Err(RedfishErr::MethodNotAllowed(collection.get_allowed_methods())),
                None => Err(RedfishErr::NotFound),
            },
            Some(resource) => match resource.delete {
                None => Err(RedfishErr::MethodNotAllowed(resource.get_allowed_methods())),
                Some(delete) => {
                    delete(resource)?;
                    if let Some(collection_uri) = &resource.collection {
                        if let Some(collection) = self.collections.get_mut(collection_uri) {
                            if let Some(member_index) = collection.members.iter().position(|x| x == uri) {
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

    fn patch(&mut self, uri: &str, req: serde_json::Value, username: Option<&str>) -> Result<&dyn RedfishNode, RedfishErr> {
        if username.is_none() {
            return Err(RedfishErr::Unauthorized);
        }
        match self.resources.get_mut(uri) {
            None => match self.collections.get(uri) {
                Some(collection) => Err(RedfishErr::MethodNotAllowed(collection.get_allowed_methods())),
                None => Err(RedfishErr::NotFound),
            },
            Some(resource) => match resource.patch {
                None => Err(RedfishErr::MethodNotAllowed(resource.get_allowed_methods())),
                Some(patch) => {
                    patch(resource, req)?;
                    Ok(resource)
                },
            },
        }
    }

    fn get_collection_types(&self) -> &[RedfishCollectionType] {
        &self.collection_types
    }

    fn get_resource_types(&self) -> &[RedfishResourceType] {
        &self.resource_types
    }
}