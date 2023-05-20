use std::collections::HashMap;
use serde_json::{Value, json, Map};
use redfish_axum::{
    RedfishNode,
    RedfishTree,
};
use redfish_data::{
    RedfishCollectionType,
    RedfishResourceType,
    RedfishCollectionSchemaVersion,
    RedfishResourceSchemaVersion,
    get_uri_id,
};

pub struct RedfishCollection {
    uri: String,
    resource_type: String, //FIXME: name conflicts with RedfishResourceType
    schema_version: RedfishCollectionSchemaVersion,
    name: String,
    pub members: Vec<String>,
    // if user should not be able to POST to collection, this should be None
    // else, it should be a function that returns new RedfishResource generated from Request
    // that function should *not* add the resource to the collection's members vector.
    post: Option<fn(&RedfishCollection, serde_json::Value) -> Result<RedfishResource, ()>>,
}

impl RedfishCollection {
    pub fn new(
        uri: &str, resource_type: String, name: String,
        members: Vec<String>,
        post: Option<fn(&RedfishCollection, serde_json::Value) -> Result<RedfishResource, ()>>,
    ) -> Self {
        Self {
            uri: String::from(uri),
            resource_type,
            schema_version: RedfishCollectionSchemaVersion::new(1),
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
            "@odata.type": format!("#{}.{}", self.resource_type, self.resource_type),
            "Name": self.name,
            "Members": member_list,
            "Members@odata.count": self.members.len(),
        })
    }

    fn can_delete(&self) -> bool { false }

    fn can_patch(&self) -> bool { false }

    fn can_post(&self) -> bool { self.post.is_some() }
}

pub struct RedfishResource {
    uri: String, //TODO: Enforce things here? Does DMTF recommend trailing slash or no?
    resource_type: String, // FIXME: Name conflicts with RedfishResourceType ?
    schema_version: RedfishResourceSchemaVersion,
    body: Map<String, Value>,
    deletable: bool,
    patchable: bool,
    collection: Option<String>,
}

impl RedfishResource {
    pub fn new(
        uri: &str,
        resource_type: String,
        schema_version: RedfishResourceSchemaVersion,
        term_name: String,
        name: String,
        deletable: bool,
        patchable: bool,
        collection: Option<String>,
        rest: Value,
    ) -> Self {
        let mut body = rest.as_object().unwrap().clone();
        body.insert(String::from("@odata.id"), json!(uri));
        body.insert(String::from("@odata.type"), json!(format!("#{}.{}.{}", resource_type, schema_version.to_str(), term_name)));
        let id = get_uri_id(uri);
        body.insert(String::from("Id"), json!(id));
        body.insert(String::from("Name"), json!(name));
        Self {
            uri: String::from(uri), resource_type, schema_version, body, deletable, patchable, collection,
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

    fn can_delete(&self) -> bool { self.deletable }

    fn can_patch(&self) -> bool { self.patchable }

    fn can_post(&self) -> bool { false }
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
        let resource_type_name = resource.resource_type.clone();
        let schema_version = resource.schema_version.clone();
        self.resources.insert(resource.uri.clone(), resource);
        for resource_type in self.resource_types.iter() {
            if resource_type.name == resource_type_name && resource_type.version == schema_version {
                return;
            }
        }
        self.resource_types.push(RedfishResourceType::new_dmtf(resource_type_name, schema_version));
    }

    pub fn add_collection(&mut self, collection: RedfishCollection) {
        let collection_type_name = collection.resource_type.clone();
        let schema_version = collection.schema_version.clone();
        self.collections.insert(collection.uri.clone(), collection);
        for collection_type in self.collection_types.iter() {
            if collection_type.name == collection_type_name && collection_type.version == schema_version {
                return;
            }
        }
        self.collection_types.push(RedfishCollectionType::new_dmtf(collection_type_name, schema_version));
    }
}

impl RedfishTree for MockTree {
    fn get(&self, uri: &str) -> Option<&dyn RedfishNode> {
        if let Some(resource) = self.resources.get(uri) {
            return Some(resource);
        }
        if let Some(collection) = self.collections.get(uri) {
            return Some(collection);
        }
        None
    }

    fn create(&mut self, uri: &str, req: serde_json::Value) -> Result<&dyn RedfishNode, ()> {
        let collection = self.collections.get_mut(uri);
        if collection.is_none() {
            return Err(());
        }
        let collection = collection.unwrap();
        match collection.post {
            None => Err(()),
            Some(post) => {
                let member = post(collection, req)?;
                let member_uri = member.uri.clone();
                self.resources.insert(member.uri.clone(), member);
                // Update members of collection.
                collection.members.push(member_uri.clone());
                // Return new resource.
                match self.get(member_uri.as_str()) {
                    Some(resource) => Ok(resource),
                    None => Err(())
                }
            }
        }
    }

    fn delete(&mut self, uri: &str) -> Result<(), ()> {
        let resource = self.resources.get(uri);
        if resource.is_none() {
            return Err(());
        }
        let resource = resource.unwrap();
        if ! resource.can_delete() {
            return Err(());
        }
        if let Some(collection_uri) = &resource.collection {
            if let Some(collection) = self.collections.get_mut(collection_uri) {
                if let Some(member_index) = collection.members.iter().position(|x| x == uri) {
                    collection.members.remove(member_index);
                }
            }
        }
        self.resources.remove(uri);
        return Ok(());
    }

    fn patch(&mut self, uri: &str, req: serde_json::Value) -> Result<&dyn RedfishNode, ()> {
        let resource = self.resources.get_mut(uri);
        if resource.is_none() {
            return Err(());
        }
        let resource = resource.unwrap();
        if ! resource.can_patch() {
            return Err(());
        }
        if uri != "/redfish/v1/SessionService" {
            return Err(());
        }
        // TODO: Move to per-resource functions
        // FIXME: Allow patch that doesn't set this! And do correct error handling!
        let new_timeout = req.as_object().unwrap().get("SessionTimeout").unwrap().as_u64().unwrap();
        resource.body["SessionTimeout"] = Value::from(new_timeout);
        return Ok(resource);
    }

    fn get_collection_types(&self) -> &[RedfishCollectionType] {
        &self.collection_types
    }

    fn get_resource_types(&self) -> &[RedfishResourceType] {
        &self.resource_types
    }
}