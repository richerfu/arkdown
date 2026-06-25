use crate::SignError;

const OID_ID_DATA: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x01];
const OID_ID_SIGNED_DATA: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x02];

pub(crate) struct ProfileContent {
    json: String,
}

impl ProfileContent {
    pub(crate) fn from_signed_profile(profile: &[u8]) -> Result<Self, SignError> {
        let content = SignedProfileContentReader::new(profile).read_content()?;
        let json = String::from_utf8(content)
            .map_err(|e| SignError::Config(format!("profile content is not UTF-8 JSON: {e}")))?;
        Ok(Self { json })
    }

    pub(crate) fn owner_id(&self) -> Result<Option<String>, SignError> {
        let value: serde_json::Value = serde_json::from_str(&self.json)
            .map_err(|e| SignError::Config(format!("profile content is not valid JSON: {e}")))?;
        let profile_type = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| SignError::Config("profile type is missing".to_string()))?;
        match profile_type {
            "debug" => Ok(Some("DEBUG_LIB_ID".to_string())),
            "release" => {
                let owner_id = value
                    .get("bundle-info")
                    .and_then(|bundle_info| bundle_info.get("app-identifier"))
                    .and_then(serde_json::Value::as_str);
                if let Some(owner_id) = owner_id {
                    if owner_id.is_empty() || owner_id.len() > 32 {
                        return Err(SignError::Config(
                            "profile app-identifier length is invalid".to_string(),
                        ));
                    }
                    Ok(Some(owner_id.to_string()))
                } else {
                    Ok(None)
                }
            }
            _ => Err(SignError::Config(format!(
                "unsupported profile type for code signing: {profile_type}"
            ))),
        }
    }

    pub(crate) fn plugin_id(&self) -> Result<String, SignError> {
        let value: serde_json::Value = serde_json::from_str(&self.json)
            .map_err(|e| SignError::Config(format!("profile content is not valid JSON: {e}")))?;
        let plugin_id = value
            .get("app-services-capabilities")
            .and_then(|capabilities| capabilities.get("ohos.permission.kernel.SUPPORT_PLUGIN"))
            .and_then(|permission| permission.get("pluginDistributionIDs"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                SignError::Config("profile pluginDistributionIDs is missing".to_string())
            })?;
        if plugin_id.is_empty() {
            return Err(SignError::Config(
                "profile pluginDistributionIDs is empty".to_string(),
            ));
        }
        Ok(plugin_id.to_string())
    }
}

struct SignedProfileContentReader<'a> {
    profile: &'a [u8],
}

impl<'a> SignedProfileContentReader<'a> {
    fn new(profile: &'a [u8]) -> Self {
        Self { profile }
    }

    fn read_content(&self) -> Result<Vec<u8>, SignError> {
        let mut reader = DerReader::new(self.profile);
        let outer = reader.read_tlv()?;
        reader.expect_end()?;
        if outer.tag != 0x30 {
            return Err(SignError::DerError(
                "profile CMS ContentInfo is not a sequence".to_string(),
            ));
        }

        let mut content_info = outer.reader();
        content_info.expect_oid(OID_ID_SIGNED_DATA, "profile CMS signedData")?;
        let signed_data_container =
            content_info.expect_tag(0xa0, "profile CMS signedData content")?;
        content_info.expect_end()?;

        let mut signed_data_container = signed_data_container.reader();
        let signed_data = signed_data_container.expect_tag(0x30, "profile CMS SignedData")?;
        signed_data_container.expect_end()?;

        let mut signed_data = signed_data.reader();
        signed_data.expect_any("profile CMS SignedData version")?;
        signed_data.expect_any("profile CMS digestAlgorithms")?;
        let encap_content_info = signed_data.expect_tag(0x30, "profile CMS encapContentInfo")?;

        let mut encap_content_info = encap_content_info.reader();
        encap_content_info.expect_oid(OID_ID_DATA, "profile CMS eContent type")?;
        let econtent = encap_content_info.expect_tag(0xa0, "profile CMS eContent")?;
        encap_content_info.expect_end()?;

        let mut econtent = econtent.reader();
        let octets = econtent.expect_tag(0x04, "profile CMS eContent octet string")?;
        econtent.expect_end()?;
        Ok(octets.value.to_vec())
    }
}

#[derive(Clone, Copy)]
struct DerElement<'a> {
    tag: u8,
    value: &'a [u8],
}

impl<'a> DerElement<'a> {
    fn reader(self) -> DerReader<'a> {
        DerReader::new(self.value)
    }
}

struct DerReader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> DerReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    fn read_tlv(&mut self) -> Result<DerElement<'a>, SignError> {
        let tag = *self
            .data
            .get(self.offset)
            .ok_or_else(|| SignError::DerError("unexpected end of DER data".to_string()))?;
        self.offset += 1;
        let len = self.read_len()?;
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| SignError::DerError("DER length overflow".to_string()))?;
        let value = self
            .data
            .get(self.offset..end)
            .ok_or_else(|| SignError::DerError("DER value extends beyond input".to_string()))?;
        self.offset = end;
        Ok(DerElement { tag, value })
    }

    fn read_len(&mut self) -> Result<usize, SignError> {
        let first = *self
            .data
            .get(self.offset)
            .ok_or_else(|| SignError::DerError("unexpected end of DER length".to_string()))?;
        self.offset += 1;
        if first & 0x80 == 0 {
            return Ok(first as usize);
        }
        let count = (first & 0x7f) as usize;
        if count == 0 || count > std::mem::size_of::<usize>() {
            return Err(SignError::DerError(
                "unsupported DER length encoding".to_string(),
            ));
        }
        let end = self
            .offset
            .checked_add(count)
            .ok_or_else(|| SignError::DerError("DER length overflow".to_string()))?;
        let bytes = self
            .data
            .get(self.offset..end)
            .ok_or_else(|| SignError::DerError("DER length extends beyond input".to_string()))?;
        self.offset = end;

        let mut len = 0usize;
        for byte in bytes {
            len = (len << 8) | (*byte as usize);
        }
        Ok(len)
    }

    fn expect_tag(&mut self, tag: u8, context: &str) -> Result<DerElement<'a>, SignError> {
        let element = self.read_tlv()?;
        if element.tag != tag {
            return Err(SignError::DerError(format!(
                "{context} has unexpected DER tag 0x{:02x}",
                element.tag
            )));
        }
        Ok(element)
    }

    fn expect_any(&mut self, context: &str) -> Result<DerElement<'a>, SignError> {
        self.read_tlv()
            .map_err(|e| SignError::DerError(format!("{context}: {e}")))
    }

    fn expect_oid(&mut self, expected: &[u8], context: &str) -> Result<(), SignError> {
        let element = self.expect_tag(0x06, context)?;
        if element.value != expected {
            return Err(SignError::DerError(format!("{context} OID mismatch")));
        }
        Ok(())
    }

    fn expect_end(&self) -> Result<(), SignError> {
        if self.offset == self.data.len() {
            Ok(())
        } else {
            Err(SignError::DerError("trailing DER data".to_string()))
        }
    }
}
