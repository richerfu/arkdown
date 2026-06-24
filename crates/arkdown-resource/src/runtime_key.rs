pub(crate) struct ResourceRuntimeKey<'a> {
    resource_type: &'a str,
    name: &'a str,
}

impl<'a> ResourceRuntimeKey<'a> {
    pub(crate) fn new(resource_type: &'a str, name: &'a str) -> Self {
        Self {
            resource_type,
            name,
        }
    }

    pub(crate) fn to_key(&self) -> String {
        let mut key = String::with_capacity(self.resource_type.len() + 1 + self.name.len());
        key.push_str(self.resource_type);
        key.push('.');
        key.push_str(self.name);
        key
    }
}

pub(crate) struct ResourceRuntimeKeyText<'a> {
    raw: &'a str,
}

impl<'a> ResourceRuntimeKeyText<'a> {
    pub(crate) fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    pub(crate) fn resource_type(&self) -> &'a str {
        self.raw
            .find('.')
            .and_then(|separator| self.raw.get(..separator))
            .unwrap_or(self.raw)
    }
}
