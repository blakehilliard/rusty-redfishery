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
    fn get_versioned_name(&self) -> String {
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
    // Create for a DMTF schema of a redfish collection
    pub fn new_dmtf(name: String, version: RedfishCollectionSchemaVersion) -> Self {
        Self {
            xml_schema_uri: format!("http://redfish.dmtf.org/schemas/v1/{}_{}.xml", name, version.to_str()),
            name,
            version,
        }
    }

    pub fn to_xml(&self) -> String {
        format!("  <edmx:Reference Uri=\"{}\">\n    <edmx:Include Namespace=\"{}\" />\n  </edmx:Reference>\n",
                self.xml_schema_uri, self.name)
    }
}