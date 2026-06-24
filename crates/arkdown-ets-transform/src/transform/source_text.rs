pub(super) struct ViewpuOutputText {
    code: String,
}

impl ViewpuOutputText {
    pub(super) fn new(code: String) -> Self {
        Self { code }
    }

    pub(super) fn without_extra_trailing_blank_lines(mut self) -> String {
        while self.code.ends_with("\n\n") {
            self.code.pop();
        }
        self.code
    }
}
