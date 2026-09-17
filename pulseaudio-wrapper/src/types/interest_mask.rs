/// A event generator a subscription may register interest with.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum InterestMask {
    Sink,
    Source,
    SinkInput,
    SourceOutput,
    Module,
    Client,
    SampleCache,
    Server,
    Card,
}

impl From<InterestMask> for libpulse_binding::context::subscribe::InterestMaskSet {
    fn from(interest: InterestMask) -> Self {
        match interest {
            InterestMask::Sink => Self::SINK,
            InterestMask::Source => Self::SOURCE,
            InterestMask::SinkInput => Self::SINK_INPUT,
            InterestMask::SourceOutput => Self::SOURCE_OUTPUT,
            InterestMask::Module => Self::MODULE,
            InterestMask::Client => Self::CLIENT,
            InterestMask::SampleCache => Self::SAMPLE_CACHE,
            InterestMask::Server => Self::SERVER,
            InterestMask::Card => Self::CARD,
        }
    }
}

impl std::ops::BitOr for InterestMask {
    type Output = InterestMaskSet;

    fn bitor(self, rhs: Self) -> Self::Output {
        InterestMaskSetBuilder::new().set(self).set(rhs).build()
    }
}

/// A set of event generators that a subscription may subscribe to.
///
/// Clients may use [`InterestMaskSetBuilder`], the bitwise or (`|`) operator, or set the fields directly to create
/// these. Implements iterating through set flags.
#[allow(clippy::struct_excessive_bools)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct InterestMaskSet {
    pub sink: bool,
    pub source: bool,
    pub sink_input: bool,
    pub source_output: bool,
    pub module: bool,
    pub client: bool,
    pub sample_cache: bool,
    pub server: bool,
    pub card: bool,
}

impl InterestMaskSet {
    /// Create a new set with all flags set to `true`.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            sink: true,
            source: true,
            sink_input: true,
            source_output: true,
            module: true,
            client: true,
            sample_cache: true,
            server: true,
            card: true,
        }
    }

    /// Create a new set with all flags set to `false`. The default.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            sink: false,
            source: false,
            sink_input: false,
            source_output: false,
            module: false,
            client: false,
            sample_cache: false,
            server: false,
            card: false,
        }
    }

    /// Whether the set contains `interest`.
    #[must_use]
    pub const fn contains(&self, interest: InterestMask) -> bool {
        match interest {
            InterestMask::Sink => self.sink,
            InterestMask::Source => self.source,
            InterestMask::SinkInput => self.sink_input,
            InterestMask::SourceOutput => self.source_output,
            InterestMask::Module => self.module,
            InterestMask::Client => self.client,
            InterestMask::SampleCache => self.sample_cache,
            InterestMask::Server => self.server,
            InterestMask::Card => self.card,
        }
    }

    /// Returns an iterator over the flags set to `true`.
    #[must_use]
    pub fn iter(&self) -> InterestMaskSetIter {
        (*self).into_iter()
    }
}

impl IntoIterator for InterestMaskSet {
    type Item = InterestMask;

    type IntoIter = InterestMaskSetIter;

    fn into_iter(self) -> Self::IntoIter {
        InterestMaskSetIter {
            entries: [
                (self.sink, InterestMask::Sink),
                (self.source, InterestMask::Source),
                (self.sink_input, InterestMask::SinkInput),
                (self.source_output, InterestMask::SourceOutput),
                (self.module, InterestMask::Module),
                (self.client, InterestMask::Client),
                (self.sample_cache, InterestMask::SampleCache),
                (self.server, InterestMask::Server),
                (self.card, InterestMask::Card),
            ],
            index: 0,
        }
    }
}

impl From<InterestMaskSet> for libpulse_binding::context::subscribe::InterestMaskSet {
    fn from(set: InterestMaskSet) -> Self {
        set.into_iter().map(Self::from).collect()
    }
}

impl From<libpulse_binding::context::subscribe::InterestMaskSet> for InterestMaskSet {
    fn from(set: libpulse_binding::context::subscribe::InterestMaskSet) -> Self {
        let all = Self::all();
        let mut builder = InterestMaskSetBuilder::new();

        for interest in all {
            if set.contains(interest.into()) {
                builder = builder.set(interest);
            }
        }

        builder.build()
    }
}

impl IntoIterator for &InterestMaskSet {
    type Item = InterestMask;
    type IntoIter = InterestMaskSetIter;

    fn into_iter(self) -> Self::IntoIter {
        (*self).into_iter()
    }
}

impl std::ops::BitOr<InterestMask> for InterestMaskSet {
    type Output = Self;

    fn bitor(self, rhs: InterestMask) -> Self::Output {
        InterestMaskSetBuilder { inner: self }.set(rhs).build()
    }
}

/// An opaque iterator for [`InterestMaskSet`].
#[derive(Debug, Clone)]
pub struct InterestMaskSetIter {
    entries: [(bool, InterestMask); 9],
    index: usize,
}

impl Iterator for InterestMaskSetIter {
    type Item = InterestMask;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.entries.len() {
            let result = self.entries.get(self.index);
            self.index = self.index.checked_add(1)?;

            if let Some((set, variant)) = result
                && *set
            {
                return Some(*variant);
            }
        }

        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.entries.len().checked_sub(self.index);
        (0, remaining)
    }
}

/// A builder for [`InterestMaskSet`].
///
/// # Examples
///
/// ```rust
/// use pulseaudio_wrapper::types::{InterestMask, InterestMaskSet, InterestMaskSetBuilder};
///
/// let only_sink = InterestMaskSetBuilder::new()
///     .set(InterestMask::Sink)
///     .build();
/// assert!(only_sink.contains(InterestMask::Sink));
/// assert!(!only_sink.contains(InterestMask::Source));
/// let all_but_sink = InterestMaskSetBuilder::from(InterestMaskSet::all())
///     .unset(InterestMask::Sink)
///     .build();
/// assert!(!all_but_sink.contains(InterestMask::Sink));
/// assert!(all_but_sink.contains(InterestMask::Source));
/// let with_set_all = InterestMaskSetBuilder::new()
///     .set_all(vec![
///         InterestMask::Sink,
///         InterestMask::Source,
///         InterestMask::SinkInput,
///     ])
///     .build();
/// assert!(with_set_all.contains(InterestMask::Sink));
/// assert!(with_set_all.contains(InterestMask::Source));
/// assert!(with_set_all.contains(InterestMask::SinkInput));
/// ```
#[derive(Default, Debug, Copy, Clone)]
pub struct InterestMaskSetBuilder {
    inner: InterestMaskSet,
}

impl InterestMaskSetBuilder {
    /// Create a new builber, defaults to [`InterestMaskSet::none`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set an interest to `true`.
    #[must_use]
    pub const fn set(mut self, interest: InterestMask) -> Self {
        match interest {
            InterestMask::Sink => self.inner.sink = true,
            InterestMask::Source => self.inner.source = true,
            InterestMask::SinkInput => self.inner.sink_input = true,
            InterestMask::SourceOutput => self.inner.source_output = true,
            InterestMask::Module => self.inner.module = true,
            InterestMask::Client => self.inner.client = true,
            InterestMask::SampleCache => self.inner.sample_cache = true,
            InterestMask::Server => self.inner.server = true,
            InterestMask::Card => self.inner.card = true,
        }

        self
    }

    /// Set a collection of interests to `true`.
    #[must_use]
    pub fn set_all(mut self, interests: impl IntoIterator<Item = InterestMask>) -> Self {
        for i in interests {
            self = self.set(i);
        }

        self
    }

    /// Set an interest to `false`.
    #[must_use]
    pub const fn unset(mut self, interest: InterestMask) -> Self {
        match interest {
            InterestMask::Sink => self.inner.sink = false,
            InterestMask::Source => self.inner.source = false,
            InterestMask::SinkInput => self.inner.sink_input = false,
            InterestMask::SourceOutput => self.inner.source_output = false,
            InterestMask::Module => self.inner.module = false,
            InterestMask::Client => self.inner.client = false,
            InterestMask::SampleCache => self.inner.sample_cache = false,
            InterestMask::Server => self.inner.server = false,
            InterestMask::Card => self.inner.card = false,
        }

        self
    }

    /// Set a collection of interests to `false`.
    #[must_use]
    pub fn unset_all(mut self, interests: impl IntoIterator<Item = InterestMask>) -> Self {
        for i in interests {
            self = self.unset(i);
        }

        self
    }

    #[must_use]
    pub const fn build(self) -> InterestMaskSet {
        self.inner
    }
}

impl From<InterestMaskSet> for InterestMaskSetBuilder {
    fn from(set: InterestMaskSet) -> Self {
        Self { inner: set }
    }
}
