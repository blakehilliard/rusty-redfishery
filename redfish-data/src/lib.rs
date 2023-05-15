#[derive(Clone, PartialEq)]
pub struct RedfishResourceSchemaVersion {
    major: u32,
    minor: u32,
    build: u32,
}

impl RedfishResourceSchemaVersion {
    pub fn new(major: u32, minor: u32, build: u32) -> Self {
        Self { major, minor, build }
    }

    pub fn to_str(&self) -> String {
        format!("v{}_{}_{}", self.major, self.minor, self.build)
    }
}

#[derive(Clone, PartialEq)]
pub struct RedfishCollectionSchemaVersion {
    version: u32,
}

impl RedfishCollectionSchemaVersion {
    pub fn new(version: u32) -> Self {
        Self { version }
    }

    pub fn to_str(&self) -> String {
        format!("v{}", self.version)
    }
}

pub struct RedfishResourceType {
    pub name: String,
    pub version: RedfishResourceSchemaVersion,
    pub xml_schema_uri: String,
}

impl RedfishResourceType {
    // Create for a DMTF schema of a redfish resource
    pub fn new_dmtf(name: String, version: RedfishResourceSchemaVersion) -> Self {
        Self {
            xml_schema_uri: format!("http://redfish.dmtf.org/schemas/v1/{}_v{}.xml", name, version.major),
            name,
            version,
        }
    }

    // TODO: This should be more commonized
    pub fn get_versioned_name(&self) -> String {
        format!("{}.{}", self.name, self.version.to_str())
    }

    pub fn to_xml(&self) -> String {
        format!("  <edmx:Reference Uri=\"{}\">\n    <edmx:Include Namespace=\"{}\" />\n    <edmx:Include Namespace=\"{}\" />\n  </edmx:Reference>\n",
                self.xml_schema_uri, self.name, self.get_versioned_name())
    }
}

pub struct RedfishCollectionType {
    pub name: String,
    pub version: RedfishCollectionSchemaVersion,
    pub xml_schema_uri: String,
}

impl RedfishCollectionType {
    pub fn new_dmtf(name: String, version: RedfishCollectionSchemaVersion) -> Self {
        Self {
            xml_schema_uri: format!("http://redfish.dmtf.org/schemas/v1/{}_{}.xml", name, version.to_str()),
            name,
            version,
        }
    }

    // All collection versons are (currently?) v1 so making this to be the less tedious option
    pub fn new_dmtf_v1(name: String) -> Self {
        RedfishCollectionType::new_dmtf(name, RedfishCollectionSchemaVersion::new(1))
    }

    pub fn to_xml(&self) -> String {
        format!("  <edmx:Reference Uri=\"{}\">\n    <edmx:Include Namespace=\"{}\" />\n  </edmx:Reference>\n",
                self.xml_schema_uri, self.name)
    }
}

pub fn get_odata_metadata_document(collection_types: &[RedfishCollectionType], resource_types: &[RedfishResourceType]) -> String {
    let mut body = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<edmx:Edmx xmlns:edmx=\"http://docs.oasis-open.org/odata/ns/edmx\" Version=\"4.0\">\n");
    let mut service_root_type: Option<&RedfishResourceType> = None;
    for collection_type in collection_types {
        body.push_str(collection_type.to_xml().as_str());
    }
    for resource_type in resource_types {
        body.push_str(resource_type.to_xml().as_str());
        if resource_type.name == "ServiceRoot" {
            service_root_type = Some(resource_type);
        }
    }
    if service_root_type.is_some() {
        body.push_str("  <edmx:DataServices>\n");
        body.push_str("    <Schema xmlns=\"http://docs.oasis-open.org/odata/ns/edm\" Namespace=\"Service\">\n");
        body.push_str(format!("      <EntityContainer Name=\"Service\" Extends=\"{}.ServiceContainer\" />\n", service_root_type.unwrap().get_versioned_name()).as_str());
        body.push_str("    </Schema>\n  </edmx:DataServices>\n");
    }
    body.push_str("</edmx:Edmx>\n");
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_schema_version() {
        let version = RedfishCollectionSchemaVersion::new(1);
        assert_eq!(version.to_str(), "v1");
    }

    #[test]
    fn resource_schema_version() {
        let version = RedfishResourceSchemaVersion::new(1, 2, 3);
        assert_eq!(version.to_str(), "v1_2_3");
    }

    #[test]
    fn dmtf_collection_type() {
        let collection_type = RedfishCollectionType::new_dmtf_v1(String::from("SessionCollection"));
        let mut exp_xml = String::from("  <edmx:Reference Uri=\"http://redfish.dmtf.org/schemas/v1/SessionCollection_v1.xml\">\n");
        exp_xml.push_str("    <edmx:Include Namespace=\"SessionCollection\" />\n");
        exp_xml.push_str("  </edmx:Reference>\n");
        assert_eq!(collection_type.to_xml(), exp_xml);
    }

    #[test]
    fn dmtf_resource_type() {
        let version = RedfishResourceSchemaVersion::new(1, 3, 0);
        let resource_type = RedfishResourceType::new_dmtf(String::from("Role"), version);
        let mut exp_xml = String::from("  <edmx:Reference Uri=\"http://redfish.dmtf.org/schemas/v1/Role_v1.xml\">\n");
        exp_xml.push_str("    <edmx:Include Namespace=\"Role\" />\n");
        exp_xml.push_str("    <edmx:Include Namespace=\"Role.v1_3_0\" />\n");
        exp_xml.push_str("  </edmx:Reference>\n");
        assert_eq!(resource_type.to_xml(), exp_xml);
    }

    #[test]
    fn odata_metaata_document() {
        let mut collection_types = Vec::new();
        collection_types.push(RedfishCollectionType::new_dmtf_v1(String::from("SessionCollection")));

        let mut resource_types = Vec::new();
        resource_types.push(RedfishResourceType::new_dmtf(String::from("ServiceRoot"), RedfishResourceSchemaVersion::new(1, 15, 0)));

        let exp_xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>
<edmx:Edmx xmlns:edmx="http://docs.oasis-open.org/odata/ns/edmx" Version="4.0">
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/SessionCollection_v1.xml">
    <edmx:Include Namespace="SessionCollection" />
  </edmx:Reference>
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/ServiceRoot_v1.xml">
    <edmx:Include Namespace="ServiceRoot" />
    <edmx:Include Namespace="ServiceRoot.v1_15_0" />
  </edmx:Reference>
  <edmx:DataServices>
    <Schema xmlns="http://docs.oasis-open.org/odata/ns/edm" Namespace="Service">
      <EntityContainer Name="Service" Extends="ServiceRoot.v1_15_0.ServiceContainer" />
    </Schema>
  </edmx:DataServices>
</edmx:Edmx>
"#);
        let doc = get_odata_metadata_document(collection_types.as_slice(), resource_types.as_slice());
        assert_eq!(doc, exp_xml);
    }
}