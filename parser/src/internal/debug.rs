use std::fmt::{Debug, Error, Formatter};

pub(crate) struct DebugSliceReference<'a, T: Debug>(pub(crate) &'a [T]);

impl<'a, T: Debug> Debug for DebugSliceReference<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.write_str("&")?;
        f.debug_list().entries(self.0.iter()).finish()
    }
}

/// Renders any map (a [`HashMap`](std::collections::HashMap) or a
/// [`BTreeMap`](std::collections::BTreeMap)) as `HashMap::from([…])`, with the
/// entries sorted by their debug representation so the output is deterministic
/// regardless of the map's own iteration order.
pub(crate) struct DebugHashMapFrom<'a, M>(pub(crate) &'a M);

impl<'a, M, K, V> Debug for DebugHashMapFrom<'a, M>
where
    &'a M: IntoIterator<Item = (&'a K, &'a V)>,
    K: Debug + 'a,
    V: Debug + 'a,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.write_str("HashMap::from(")?;

        let mut sorted: Vec<_> = self.0.into_iter().collect();
        sorted.sort_by_key(|(k, _)| format!("{:?}", k));

        f.debug_list()
            .entries(sorted.iter().map(|(k, v)| (k, v)))
            .finish()?;

        f.write_str(")")
    }
}
