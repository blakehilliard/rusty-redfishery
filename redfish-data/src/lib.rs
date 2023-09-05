use serde::Serialize;
use serde_json::{json, Map, Value};
use std::str::FromStr;
use std::{collections::HashMap, fmt, fs};
use strum::{Display, EnumString};

#[derive(Clone, Debug, Display, PartialEq, EnumString)]
pub enum Health {
    #[strum()]
    OK,
    Warning,
    Critical,
}

#[derive(Clone, Copy, Debug)]
pub struct AllowedMethods {
    pub delete: bool,
    pub get: bool,
    pub patch: bool,
    pub post: bool,
}

impl fmt::Display for AllowedMethods {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut val = Vec::new();
        if self.get {
            val.push("GET");
            val.push("HEAD");
        }
        if self.delete {
            val.push("DELETE");
        }
        if self.patch {
            val.push("PATCH");
        }
        if self.post {
            val.push("POST");
        }
        write!(f, "{}", val.join(","))
    }
}

pub trait SchemaVersion: fmt::Display {}

#[derive(Clone, PartialEq)]
pub struct ResourceSchemaVersion {
    major: u32,
    minor: u32,
    build: u32,
}

impl ResourceSchemaVersion {
    pub fn new(major: u32, minor: u32, build: u32) -> Self {
        Self {
            major,
            minor,
            build,
        }
    }
}

impl SchemaVersion for ResourceSchemaVersion {}

impl fmt::Display for ResourceSchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}_{}_{}", self.major, self.minor, self.build)
    }
}

pub fn get_resource_odata_type(
    schema_name: &str,
    schema_version: &ResourceSchemaVersion,
    term_name: &str,
) -> String {
    format!(
        "#{}.{}.{}",
        schema_name,
        schema_version.to_string(),
        term_name
    )
}

#[derive(Clone, PartialEq)]
pub struct CollectionSchemaVersion {
    version: u32,
}

impl CollectionSchemaVersion {
    pub fn new(version: u32) -> Self {
        Self { version }
    }
}

impl SchemaVersion for CollectionSchemaVersion {}

impl fmt::Display for CollectionSchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}", self.version)
    }
}

#[derive(Clone, PartialEq)]
pub struct ResourceType {
    pub name: String,
    pub version: ResourceSchemaVersion,
    pub xml_schema_uri: String,
    pub described_by: String,
}

impl ResourceType {
    // Create for a DMTF schema of a redfish resource
    pub fn new_dmtf(name: String, version: ResourceSchemaVersion) -> Self {
        Self {
            xml_schema_uri: format!(
                "http://redfish.dmtf.org/schemas/v1/{}_v{}.xml",
                name, version.major
            ),
            described_by: format!(
                "https://redfish.dmtf.org/schemas/v1/{}.{}.json",
                name,
                version.to_string()
            ),
            name,
            version,
        }
    }

    fn get_versioned_name(&self) -> String {
        get_versioned_name(&self.name, &self.version)
    }

    pub fn to_xml(&self) -> String {
        format!("  <edmx:Reference Uri=\"{}\">\n    <edmx:Include Namespace=\"{}\" />\n    <edmx:Include Namespace=\"{}\" />\n  </edmx:Reference>\n",
                self.xml_schema_uri, self.name, self.get_versioned_name())
    }
}

#[derive(Clone, PartialEq)]
pub struct CollectionType {
    pub name: String,
    pub version: CollectionSchemaVersion,
    pub xml_schema_uri: String,
    pub described_by: String,
}

impl CollectionType {
    pub fn new_dmtf(name: String, version: CollectionSchemaVersion) -> Self {
        Self {
            xml_schema_uri: format!(
                "http://redfish.dmtf.org/schemas/v1/{}_{}.xml",
                name,
                version.to_string()
            ),
            described_by: format!("https://redfish.dmtf.org/schemas/v1/{}.json", name),
            name,
            version,
        }
    }

    // All collection versons are (currently?) v1 so making this to be the less tedious option
    pub fn new_dmtf_v1(name: String) -> Self {
        CollectionType::new_dmtf(name, CollectionSchemaVersion::new(1))
    }

    pub fn to_xml(&self) -> String {
        format!("  <edmx:Reference Uri=\"{}\">\n    <edmx:Include Namespace=\"{}\" />\n  </edmx:Reference>\n",
                self.xml_schema_uri, self.name)
    }
}

#[derive(Serialize)]
struct ODataServiceValue {
    kind: String,
    name: String,
    url: String,
}

impl ODataServiceValue {
    fn new(url: &str) -> Self {
        Self {
            kind: String::from("Singleton"),
            name: String::from(
                std::path::Path::new(url)
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap(),
            ),
            url: String::from(url),
        }
    }
}

pub fn get_odata_service_document(service_root: &Map<String, Value>) -> Map<String, Value> {
    let mut values = Vec::new();
    values.push(ODataServiceValue::new("/redfish/v1"));

    for val in service_root.values() {
        let val = val.as_object();
        if val.is_some() {
            let val = val.unwrap();
            if val.contains_key("@odata.id") {
                values.push(ODataServiceValue::new(val["@odata.id"].as_str().unwrap()));
            }
        }
    }

    let mut res = Map::new();
    res.insert(
        String::from("@odata.id"),
        Value::String(String::from("/redfish/v1/odata")),
    );
    res.insert(
        String::from("@odata.context"),
        Value::String(String::from("/redfish/v1/$metadata")),
    );
    res.insert(String::from("value"), json!(values));
    res
}

pub fn get_odata_metadata_document(
    collection_types: &[CollectionType],
    resource_types: &[ResourceType],
) -> String {
    let mut body = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<edmx:Edmx xmlns:edmx=\"http://docs.oasis-open.org/odata/ns/edmx\" Version=\"4.0\">\n");
    let mut service_root_type: Option<&ResourceType> = None;
    for collection_type in collection_types {
        body.push_str(collection_type.to_xml().as_str());
    }
    for resource_type in resource_types {
        body.push_str(resource_type.to_xml().as_str());
        if resource_type.name == "ServiceRoot" {
            service_root_type = Some(resource_type);
        }
    }
    body.push_str(
        "  <edmx:Reference Uri=\"http://redfish.dmtf.org/schemas/v1/RedfishExtensions_v1.xml\">\n",
    );
    body.push_str("    <edmx:Include Namespace=\"RedfishExtensions.v1_0_0\" Alias=\"Redfish\"/>\n");
    body.push_str("  </edmx:Reference>\n");
    if service_root_type.is_some() {
        body.push_str("  <edmx:DataServices>\n");
        body.push_str("    <Schema xmlns=\"http://docs.oasis-open.org/odata/ns/edm\" Namespace=\"Service\">\n");
        body.push_str(
            format!(
                "      <EntityContainer Name=\"Service\" Extends=\"{}.ServiceContainer\" />\n",
                service_root_type.unwrap().get_versioned_name()
            )
            .as_str(),
        );
        body.push_str("    </Schema>\n  </edmx:DataServices>\n");
    }
    body.push_str("</edmx:Edmx>\n");
    body
}

pub fn get_uri_id(uri: &str) -> String {
    match uri {
        "/redfish/v1" => String::from("RootService"),
        _ => String::from(
            std::path::Path::new(uri)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap(),
        ),
    }
}

pub fn get_versioned_name(name: &str, version: &dyn SchemaVersion) -> String {
    format!("{}.{}", name, version.to_string())
}

pub struct ErrorResponse {
    code: String,
    message: String,
    extended_info: Vec<Message>,
}

impl ErrorResponse {
    pub fn from_registry(
        registry: &MessageRegistry,
        key: &str,
        message_args: &Vec<String>,
        extended_info: Vec<Message>,
    ) -> Self {
        let message_definition = registry.get_message_definition(key).unwrap();
        Self {
            code: registry.get_message_id(key),
            message: message_definition.get_message(message_args),
            extended_info,
        }
    }

    pub fn to_json(&self) -> Map<String, Value> {
        let mut extended_info = Vec::new();
        for message in &self.extended_info {
            extended_info.push(Value::Object(message.to_json()));
        }

        let mut error = Map::new();
        error.insert(String::from("code"), Value::String(self.code.clone()));
        error.insert(String::from("message"), Value::String(self.message.clone()));
        error.insert(
            String::from("@Message.ExtendedInfo"),
            Value::Array(extended_info),
        );

        let mut res = Map::new();
        res.insert(String::from("error"), Value::Object(error));
        res
    }
}

// FIXME: Should we have unique error enum per function call ???
#[derive(Debug)]
pub enum RegistryError {
    MessageNotInRegistry,
    WrongNumberOfMessageArgs,
}

// TODO: How to avoid implicit revlock to Message schema version at the time I write this?
pub struct Message {
    // TODO: Allow OEM? How?
    version: ResourceSchemaVersion,
    id: String,
    related_properties: Vec<String>,
    message: String,
    // TODO: Is it valid to have something other than strings as message args?
    message_args: Vec<String>,
    severity: Health,
    resolution: String,
}

impl Message {
    pub fn from_registry(
        registry: &MessageRegistry,
        key: &str,
        version: ResourceSchemaVersion,
        message_args: Vec<String>,
        related_properties: Vec<String>,
    ) -> Result<Self, RegistryError> {
        let message_definition = registry
            .get_message_definition(key)
            .ok_or(RegistryError::MessageNotInRegistry)?;
        let id = registry.get_message_id(key);
        let message = message_definition.get_message(&message_args);
        Ok(Self {
            version,
            id,
            related_properties,
            message,
            message_args,
            severity: message_definition.severity.clone(),
            resolution: message_definition.resolution.clone(),
        })
    }

    //TODO: Give option to include deprecated Severity?
    //TODO: If I want to provide different variations of this, give more specific names?
    pub fn to_json(&self) -> Map<String, Value> {
        let mut res = Map::new();
        res.insert(
            String::from("@odata.type"),
            Value::String(get_resource_odata_type("Message", &self.version, "Message")),
        );
        res.insert(String::from("MessageId"), Value::String(self.id.clone()));
        res.insert(String::from("Message"), Value::String(self.message.clone()));
        res.insert(
            String::from("RelatedProperties"),
            serde_json::to_value(self.related_properties.clone()).unwrap(),
        );
        res.insert(
            String::from("MessageArgs"),
            serde_json::to_value(self.message_args.clone()).unwrap(),
        );
        res.insert(
            String::from("MessageSeverity"),
            Value::String(self.severity.to_string()),
        );
        res.insert(
            String::from("Resolution"),
            Value::String(self.resolution.clone()),
        );
        res
    }
}

pub struct MessageDefinition {
    message: String,
    severity: Health,
    number_of_args: u64,
    resolution: String,
}

impl MessageDefinition {
    fn from_registry(data: &Map<String, Value>) -> Self {
        Self {
            message: String::from(data.get("Message").unwrap().as_str().unwrap()),
            severity: Health::from_str(data.get("MessageSeverity").unwrap().as_str().unwrap())
                .unwrap(),
            number_of_args: data.get("NumberOfArgs").unwrap().as_u64().unwrap(),
            resolution: String::from(data.get("Resolution").unwrap().as_str().unwrap()),
        }
    }

    fn get_message(&self, message_args: &Vec<String>) -> String {
        let mut message = self.message.clone();
        //FIXME: Assert right number of args
        for (idx, arg) in message_args.iter().enumerate() {
            //FIXME: Ensure this finds something?
            let from = format!("%{}", idx + 1);
            message = message.replace(&from, arg);
        }
        message
    }
}

pub struct MessageRegistry {
    prefix: String,
    version: ResourceSchemaVersion,
    message_definitions: HashMap<String, MessageDefinition>,
}

impl MessageRegistry {
    pub fn from_file(path: &str) -> Self {
        let data = fs::read_to_string(path).expect("Unable to read file");
        let data: Map<String, Value> =
            serde_json::from_str(&data).expect("Unable to parse message registry file");
        let version_str = data.get("RegistryVersion").unwrap().as_str().unwrap();
        let version_parts: Vec<&str> = version_str.split(".").collect();
        let mut message_definitions = HashMap::new();
        for msg in data.get("Messages").unwrap().as_object().unwrap() {
            let msg_name = msg.0.clone();
            let msg_data = msg.1.as_object().unwrap();
            let msg_def = MessageDefinition::from_registry(msg_data);
            message_definitions.insert(msg_name, msg_def);
        }
        Self {
            prefix: String::from(data.get("RegistryPrefix").unwrap().as_str().unwrap()),
            version: ResourceSchemaVersion::new(
                version_parts[0].parse().unwrap(),
                version_parts[1].parse().unwrap(),
                version_parts[2].parse().unwrap(),
            ),
            message_definitions,
        }
    }

    pub fn get_message_definition(&self, key: &str) -> Option<&MessageDefinition> {
        self.message_definitions.get(key)
    }

    pub fn get_message_id(&self, key: &str) -> String {
        format!(
            "{}.{}.{}.{}",
            self.prefix, self.version.major, self.version.minor, key
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn get_base_registry() -> MessageRegistry {
        let mut path = env::var("CARGO_MANIFEST_DIR").unwrap();
        path.push_str("/../dmtf/Base.1.16.0.json");
        MessageRegistry::from_file(&path)
    }

    #[test]
    fn message_registry() {
        let registry = get_base_registry();
        assert_eq!(registry.prefix, String::from("Base"));
        assert_eq!(registry.version.major, 1);
        assert_eq!(registry.version.minor, 16);
        assert_eq!(registry.version.build, 0);
        let success = registry.message_definitions.get("Success").unwrap();
        assert_eq!(success.severity, Health::OK);
    }

    #[test]
    fn message() {
        let registry = get_base_registry();
        let message = Message::from_registry(
            &registry,
            "PropertyValueTypeError",
            ResourceSchemaVersion::new(1, 1, 2),
            vec![String::from("300"), String::from("SessionTimeout")],
            vec![String::from("/SessionTimeout")],
        )
        .unwrap();
        let jsonified = message.to_json();
        assert_eq!(&jsonified, json!({
            "@odata.type": "#Message.v1_1_2.Message",
            "MessageId": "Base.1.16.PropertyValueTypeError",
            "RelatedProperties": ["/SessionTimeout"],
            "Message": "The value '300' for the property SessionTimeout is of a different type than the property can accept.",
            "MessageArgs": ["300", "SessionTimeout"],
            "MessageSeverity": "Warning",
            "Resolution": "Correct the value for the property in the request body and resubmit the request if the operation failed."
        }).as_object().unwrap());
    }

    #[test]
    fn error_response() {
        let registry = get_base_registry();
        let message = Message::from_registry(
            &registry,
            "PropertyValueTypeError",
            ResourceSchemaVersion::new(1, 1, 2),
            vec![String::from("300"), String::from("SessionTimeout")],
            vec![String::from("/SessionTimeout")],
        )
        .unwrap();
        let error = ErrorResponse::from_registry(&registry, "GeneralError", &vec![], vec![message]);
        assert_eq!(&error.to_json(), json!({
            "error": {
                "code": "Base.1.16.GeneralError",
                "message": "A general error has occurred.  See Resolution for information on how to resolve the error, or @Message.ExtendedInfo if Resolution is not provided.",
                "@Message.ExtendedInfo": [
                    {
                        "@odata.type": "#Message.v1_1_2.Message",
                        "MessageId": "Base.1.16.PropertyValueTypeError",
                        "RelatedProperties": ["/SessionTimeout"],
                        "Message": "The value '300' for the property SessionTimeout is of a different type than the property can accept.",
                        "MessageArgs": ["300", "SessionTimeout"],
                        "MessageSeverity": "Warning",
                        "Resolution": "Correct the value for the property in the request body and resubmit the request if the operation failed."
                    }
                ]
            }
        }).as_object().unwrap());
    }

    #[test]
    fn uri_id() {
        assert_eq!(get_uri_id("/redfish/v1"), String::from("RootService"));
        assert_eq!(get_uri_id("/redfish/v1/Chassis"), String::from("Chassis"));
    }

    #[test]
    fn collection_schema_version() {
        let version = CollectionSchemaVersion::new(1);
        assert_eq!(version.to_string(), "v1");
    }

    #[test]
    fn resource_schema_version() {
        let version = ResourceSchemaVersion::new(1, 2, 3);
        assert_eq!(version.to_string(), "v1_2_3");
    }

    #[test]
    fn dmtf_collection_type() {
        let collection_type = CollectionType::new_dmtf_v1(String::from("SessionCollection"));
        let mut exp_xml = String::from("  <edmx:Reference Uri=\"http://redfish.dmtf.org/schemas/v1/SessionCollection_v1.xml\">\n");
        exp_xml.push_str("    <edmx:Include Namespace=\"SessionCollection\" />\n");
        exp_xml.push_str("  </edmx:Reference>\n");
        assert_eq!(collection_type.to_xml(), exp_xml);
    }

    #[test]
    fn dmtf_resource_type() {
        let version = ResourceSchemaVersion::new(1, 3, 0);
        let resource_type = ResourceType::new_dmtf(String::from("Role"), version);
        let mut exp_xml = String::from(
            "  <edmx:Reference Uri=\"http://redfish.dmtf.org/schemas/v1/Role_v1.xml\">\n",
        );
        exp_xml.push_str("    <edmx:Include Namespace=\"Role\" />\n");
        exp_xml.push_str("    <edmx:Include Namespace=\"Role.v1_3_0\" />\n");
        exp_xml.push_str("  </edmx:Reference>\n");
        assert_eq!(resource_type.to_xml(), exp_xml);
    }

    #[test]
    fn odata_service_document() {
        let service_root = json!({
            "AccountService": {
                "@odata.id": "/redfish/v1/AccountService",
            },
            "Links": {
                "Sessions": {
                    "@odata.id": "/redfish/v1/SessionService/Sessions"
                },
            },
            "RedfishVersion": "1.16.1",
            "ProtocolFeaturesSupported": {},
        });
        let doc = get_odata_service_document(service_root.as_object().unwrap());
        assert_eq!(
            doc,
            *json!({
                "@odata.id": "/redfish/v1/odata",
                "@odata.context": "/redfish/v1/$metadata",
                "value": [
                    {
                        "kind": "Singleton",
                        "name": "v1",
                        "url": "/redfish/v1",
                    },
                    {
                        "kind": "Singleton",
                        "name": "AccountService",
                        "url": "/redfish/v1/AccountService",
                    },
                ],
            })
            .as_object()
            .unwrap()
        );
    }

    #[test]
    fn odata_metadata_document() {
        let mut collection_types = Vec::new();
        collection_types.push(CollectionType::new_dmtf_v1(String::from(
            "SessionCollection",
        )));

        let mut resource_types = Vec::new();
        resource_types.push(ResourceType::new_dmtf(
            String::from("ServiceRoot"),
            ResourceSchemaVersion::new(1, 15, 0),
        ));

        let exp_xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<edmx:Edmx xmlns:edmx="http://docs.oasis-open.org/odata/ns/edmx" Version="4.0">
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/SessionCollection_v1.xml">
    <edmx:Include Namespace="SessionCollection" />
  </edmx:Reference>
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/ServiceRoot_v1.xml">
    <edmx:Include Namespace="ServiceRoot" />
    <edmx:Include Namespace="ServiceRoot.v1_15_0" />
  </edmx:Reference>
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/RedfishExtensions_v1.xml">
    <edmx:Include Namespace="RedfishExtensions.v1_0_0" Alias="Redfish"/>
  </edmx:Reference>
  <edmx:DataServices>
    <Schema xmlns="http://docs.oasis-open.org/odata/ns/edm" Namespace="Service">
      <EntityContainer Name="Service" Extends="ServiceRoot.v1_15_0.ServiceContainer" />
    </Schema>
  </edmx:DataServices>
</edmx:Edmx>
"#,
        );
        let doc =
            get_odata_metadata_document(collection_types.as_slice(), resource_types.as_slice());
        assert_eq!(doc, exp_xml);
    }
}
