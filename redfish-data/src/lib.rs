use std::fmt;

use serde::Serialize;
use serde_json::{json, Map, Value};

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

#[cfg(test)]
mod tests {
    use super::*;

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
