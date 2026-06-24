pub(crate) struct ProfileResourceRef<'a> {
    name: &'a str,
}

impl<'a> ProfileResourceRef<'a> {
    const PREFIX: &'static str = "$profile:";

    pub(crate) fn parse(resource: &'a str) -> Option<Self> {
        let name = resource.strip_prefix(Self::PREFIX)?;
        (!name.is_empty()).then_some(Self { name })
    }

    pub(crate) fn name(&self) -> &'a str {
        self.name
    }

    pub(crate) fn profile_name_or_bare(resource: &'a str) -> Option<&'a str> {
        if let Some(profile_ref) = Self::parse(resource) {
            return Some(profile_ref.name());
        }
        let name = resource.trim();
        (!name.is_empty()).then_some(name)
    }

    pub(crate) fn resource_name(name: &str) -> String {
        format!("{}{name}", Self::PREFIX)
    }
}
