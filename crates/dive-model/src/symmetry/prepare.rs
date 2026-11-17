//! Tool-level symmetry preparation and concrete-state canonical keys.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use dive_config::{ConstantEntry, ModelConfig};
use dive_eval::{ConstBinding, St};
use dive_ir::{Arg, Expr, ExprId, OpRef, StdOp};
use dive_value::{Atom, Ctx, SetKind, Value, ValueOrder};

use super::{Declaration, Group, Limits, Mode, Permutation, symmetry_runtime_error};
use crate::{BindError, Ec, Message, Model, Pred};

/// Complete pure tool-level preparation, before attachment to a safety checker.
#[derive(Clone, Debug)]
pub struct Prepared {
    /// The bounded closure; `None` means TLC's too-small heuristic disabled it.
    pub reduction: Option<Arc<Reduction>>,
    /// Startup warnings, in TLC's order.
    pub warnings: Vec<Message>,
}

/// A bounded symmetry closure over the model values it mentions.
/// State canonicalization changes keys only: the checker explores and retains
/// the original concrete state and predecessor.
#[derive(Clone, Debug)]
pub struct Reduction {
    universe: Vec<Atom>,
    indices: HashMap<Atom, usize>,
    group: Group,
    /// Per map: whether it maps every model value to one of the same TLC type,
    /// so that the image of any value has that value's comparability shape.
    typed: Vec<bool>,
    /// Identifies this closure's entries in the per-thread image cache.
    id: u64,
}

/// Set elements' images under every map of one reduction, per thread. The
/// images of the elements of a state's sets are compared, never kept or
/// evaluated, and only for elements in which no set occurs, so a shared image
/// is indistinguishable from a freshly built one. `None` marks a map whose
/// image fails to build: that element is permuted afresh, which raises the
/// error where the uncached search would.
///
/// Every cached element and image also has a rank: ranks are ordered as
/// their values are by the total order, and equal values share one, so that
/// comparing two sets of them is comparing integers.
#[derive(Default)]
struct ImageCache {
    owner: Option<(u64, Ctx)>,
    images: HashMap<Exact, Arc<Entry>>,
    held: usize,
    /// Bumped whenever the cache starts over, invalidating every rank id.
    generation: u64,
    /// The distinct ranked values in ascending order, with their rank ids.
    sorted: Vec<(Value, u32)>,
    /// Rank by rank id. Renumbering keeps the order of existing ranks.
    rank: Vec<u64>,
}

/// One element's rank id, and its images (with rank ids) under every map.
struct Entry {
    key: u32,
    images: Box<[Option<(Value, u32)>]>,
}

/// The space left between consecutive ranks when they are renumbered.
const RANK_GAP: u64 = 1 << 24;

impl ImageCache {
    /// The rank id of `value`, ranking it if it is new.
    fn rank_id(&mut self, value: &Value, order: ValueOrder) -> u32 {
        match self
            .sorted
            .binary_search_by(|(probe, _)| probe.cmp_with(value, order))
        {
            Ok(at) => self.sorted[at].1,
            Err(at) => {
                let id = u32::try_from(self.rank.len()).unwrap_or(u32::MAX);
                let below = at
                    .checked_sub(1)
                    .map_or(0, |i| self.rank[self.sorted[i].1 as usize]);
                let above = self
                    .sorted
                    .get(at)
                    .map_or(u64::MAX, |(_, above)| self.rank[*above as usize]);
                self.rank.push(below + (above - below) / 2);
                self.sorted.insert(at, (value.clone(), id));
                if above - below < 2 {
                    for (i, (_, id)) in self.sorted.iter().enumerate() {
                        self.rank[*id as usize] = (i as u64 + 1) * RANK_GAP;
                    }
                }
                id
            }
        }
    }
}

/// A cache key: equal values that TLC also represents alike. Equal values of
/// different TLC classes (a record and a string-domain function) compare
/// alike only where neither comparison fails, so they never share images.
struct Exact(Value);

impl PartialEq for Exact {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && same_classes(&self.0, &other.0)
    }
}

impl Eq for Exact {}

impl std::hash::Hash for Exact {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.hash64());
    }
}

/// Whether two equal set-free values have the same TLC classes throughout.
fn same_classes(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Fcn(f), Value::Fcn(g)) => {
            f.ptr_eq(g)
                || (f.kind() == g.kind()
                    && f.is_record_literal() == g.is_record_literal()
                    && f.is_tuple_literal() == g.is_tuple_literal()
                    && f.len() == g.len()
                    && f.iter()
                        .zip(g.iter())
                        .all(|((d, v), (e, w))| same_classes(&d, &e) && same_classes(v, w)))
        }
        (Value::Int(_) | Value::BigInt(_), Value::Int(_) | Value::BigInt(_)) => true,
        _ => std::mem::discriminant(a) == std::mem::discriminant(b),
    }
}

/// The most images a thread's cache holds before it starts over.
const IMAGE_CACHE_LIMIT: usize = 1 << 18;

thread_local! {
    static IMAGES: std::cell::RefCell<ImageCache> = std::cell::RefCell::default();
}

static NEXT_REDUCTION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl Model {
    /// Prepares a pure declaration using `Tool.getSymmetryPerms`'s ordering:
    /// representations, too-small warnings, domain/range checks, closure,
    /// then unused argument warnings. `cfg` must be the bound configuration.
    /// This API does not itself activate reduction or remove checker refusals.
    ///
    /// # Example
    /// ```
    /// # use std::sync::Arc;
    /// # use dive_model::{BindOptions, bind};
    /// # use dive_model::symmetry::{Mode, Limits};
    /// let source = "---- MODULE M ----\nEXTENDS TLC\nCONSTANT S\nVARIABLE x\n\
    ///               Init == x = TRUE\nNext == UNCHANGED x\n\
    ///               Sym == Permutations(S)\n====\n";
    /// let analysis = dive_sema::analyze_source("M.tla", source, &dive_sema::Options::default());
    /// let ir = Arc::new(dive_ir::lower(&analysis).unwrap());
    /// let cfg = dive_config::parse("CONSTANT S={a,b}\nINIT Init\nNEXT Next\nSYMMETRY Sym\n")
    ///     .into_config().unwrap();
    /// let model = bind(ir, &cfg, &BindOptions::tlc()).unwrap();
    /// let prepared = model.prepare_symmetry(&cfg, Mode::Tlc, Limits::default()).unwrap().unwrap();
    /// assert!(prepared.reduction.is_some());
    /// assert!(prepared.warnings.is_empty());
    /// ```
    ///
    /// # Errors
    /// Invalid declarations, unavailable warning source evidence, unverified
    /// startup exceptions, or a bounded-closure refusal.
    pub fn prepare_symmetry(
        &self,
        cfg: &ModelConfig,
        mode: Mode,
        limits: Limits,
    ) -> Result<Option<Prepared>, BindError> {
        let Some(declaration) = self.symmetry_declaration()? else {
            return Ok(None);
        };
        let args = argument_nodes(self.ir.expr(declaration.predicate.expr))?;
        if !args.is_empty() && args.len() >= declaration.functions.len() {
            // The default profile rejects non-bijective model-value maps even
            // when TLC disables them. Non-model identities retain TLC's warning
            // shortcut, deliberately: rejecting them too would be a further
            // result-changing default.
            // A duplicate generator supplies the kernel's capacity precondition
            // without altering its closure; no reduction is attached here.
            if mode == Mode::Checked
                && !declaration.functions.is_empty()
                && declaration.functions.iter().all(|value| match value {
                    Value::Fcn(function) => function.iter().all(|(domain, range)| {
                        matches!(domain, Value::Model(_)) && matches!(range, Value::Model(_))
                    }),
                    _ => false,
                })
            {
                let mut checked = declaration.clone();
                if checked.functions.len() == 1 {
                    checked.functions =
                        vec![checked.functions[0].clone(), checked.functions[0].clone()].into();
                }
                prepare_group(&checked, mode, limits)?;
            }
            return Ok(Some(Prepared {
                reduction: None,
                warnings: vec![warning(self, cfg, &args)?],
            }));
        }
        let reduction = prepare_group(&declaration, mode, limits)?;
        let members: HashSet<Atom> = reduction
            .group
            .maps()
            .iter()
            .flat_map(|map| {
                map.images()
                    .iter()
                    .enumerate()
                    .filter(|(index, image)| index != *image)
                    .map(|(_, image)| reduction.universe[*image].clone())
            })
            .collect();
        let mut unused = Vec::new();
        for arg in args {
            if let Some(values) = permutation_argument(self, &declaration, arg)?
                && !values
                    .iter()
                    .any(|value| matches!(value, Value::Model(m) if members.contains(m)))
            {
                unused.push(arg);
            }
        }
        Ok(Some(Prepared {
            reduction: Some(Arc::new(reduction)),
            warnings: if unused.is_empty() {
                Vec::new()
            } else {
                vec![warning(self, cfg, &unused)?]
            },
        }))
    }
}

fn argument_nodes(expr: &Expr) -> Result<Vec<ExprId>, BindError> {
    match expr {
        Expr::Def(_) | Expr::Const(_) | Expr::Bool(_) | Expr::Num(_) | Expr::Str(_) => {
            Ok(Vec::new())
        }
        Expr::Builtin { args, .. }
        | Expr::SetEnum(args)
        | Expr::Tuple(args)
        | Expr::Cartesian(args) => Ok(args.to_vec()),
        Expr::Apply { args, .. } => args
            .iter()
            .map(|arg| match arg {
                Arg::Expr { expr: id, .. } => Ok(*id),
                Arg::Op { .. } => Err(BindError::unsupported(
                    "SYMMETRY warning arguments containing an operator argument",
                )),
            })
            .collect(),
        Expr::If { cond, then, other } => Ok(vec![*cond, *then, *other]),
        Expr::Junction { items, .. } => Ok(items.to_vec()),
        _ => Err(BindError::unsupported(
            "SYMMETRY declaration syntax whose TLC argument/warning shape is not implemented",
        )),
    }
}

fn warning(model: &Model, cfg: &ModelConfig, args: &[ExprId]) -> Result<Message, BindError> {
    let mut names = Vec::new();
    for arg in args {
        let image = model
            .ir
            .human_readable_image(*arg)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                BindError::unsupported("SYMMETRY warning lacks original source tokens")
            })?;
        let alias = if image.starts_with("Permutations") {
            image
                .get("Permutations".len() + 1..image.len().saturating_sub(1))
                .ok_or_else(|| BindError::unsupported("SYMMETRY warning permutation image"))?
        } else {
            &image
        };
        let original = cfg
            .constants
            .iter()
            .rev()
            .find_map(|entry| match entry {
                ConstantEntry::Substitution {
                    name,
                    target,
                    module: None,
                } if target.text == alias => Some(name.text.as_str()),
                _ => None,
            })
            .unwrap_or(alias);
        names.push(original.to_owned());
    }
    let many = args.len() > 1;
    let parameters = [
        if many { "s" } else { "" },
        &names.join(", and "),
        if many { "have" } else { "has" },
        if many { "" } else { "s" },
    ];
    Ok(Message::Warning {
        code: Ec::TlcSymmetrySetTooSmall,
        message: Ec::TlcSymmetrySetTooSmall.render(
            &parameters
                .iter()
                .map(|s| (*s).to_owned())
                .collect::<Vec<_>>(),
        ),
    })
}

fn permutation_argument(
    model: &Model,
    declaration: &Declaration,
    arg: ExprId,
) -> Result<Option<Arc<[Value]>>, BindError> {
    let Expr::Apply { callee, args } = model.ir.expr(arg) else {
        return Ok(None);
    };
    let op = match callee {
        OpRef::Def(def) => model.ir.def(*def).std,
        OpRef::Std(op) => Some(*op),
        _ => None,
    };
    if op != Some(StdOp::TlcPermutations) {
        return Ok(None);
    }
    let [Arg::Expr { expr: domain, .. }] = args.as_ref() else {
        return Ok(None);
    };
    let value = match model.ir.expr(*domain) {
        Expr::Const(constant) => match model.evaluator.constant_binding(*constant) {
            Some(ConstBinding::Value(value)) => value,
            _ => return Ok(None),
        },
        Expr::Def(_) => model
            .eval(
                &Pred::new(*domain, declaration.predicate.scope.clone()),
                &St::constant(),
            )
            .map_err(|error| {
                BindError::unsupported(format!("SYMMETRY unused argument: {}", error.message()))
            })?,
        Expr::SubstRef { .. } => {
            return Err(BindError::unsupported(
                "SYMMETRY unused-set warning under instance substitutions",
            ));
        }
        _ => return Ok(None),
    };
    if let Value::Set(set) = value
        && matches!(set.kind(), SetKind::Enum | SetKind::Raw)
    {
        return set
            .elements(model.evaluator.options().ctx)
            .map(Some)
            .map_err(|error| {
                BindError::unsupported(format!(
                    "SYMMETRY unused-set normalization: {}",
                    error.message()
                ))
            });
    }
    Ok(None)
}

fn prepare_group(
    declaration: &Declaration,
    mode: Mode,
    limits: Limits,
) -> Result<Reduction, BindError> {
    let mut universe = Vec::new();
    let mut indices = HashMap::new();
    let mut generators = Vec::new();
    let mut work = 0usize;
    for value in declaration.functions.iter() {
        let Value::Fcn(fcn) = value else {
            return Err(BindError::bug(
                "validated symmetry function ceased to be a function",
            ));
        };
        let mut pairs = Vec::new();
        for (domain, range) in fcn.iter() {
            work = work
                .checked_add(1)
                .ok_or_else(|| BindError::unsupported("SYMMETRY generator work limit"))?;
            if work > limits.work {
                return Err(BindError::unsupported("SYMMETRY generator work limit"));
            }
            let (Value::Model(domain), Value::Model(range)) = (&domain, range) else {
                return Err(symmetry_runtime_error(
                    "Symmetry function must have model values as domain and range.",
                ));
            };
            let mut ids = [0usize; 2];
            for (at, atom) in [domain, range].into_iter().enumerate() {
                ids[at] = if let Some(index) = indices.get(atom) {
                    *index
                } else {
                    if universe.len() >= limits.model_values {
                        return Err(BindError::unsupported("SYMMETRY model-value limit"));
                    }
                    let index = universe.len();
                    universe.push(atom.clone());
                    indices.insert(atom.clone(), index);
                    index
                };
            }
            pairs.push((ids[0], ids[1]));
        }
        generators.push(pairs);
    }
    let group =
        Group::from_generators(universe.len(), &generators, mode, limits).map_err(|error| {
            BindError::unsupported(format!("SYMMETRY {}: {error}", declaration.name))
        })?;
    let typed = group
        .maps()
        .iter()
        .map(|map| {
            map.images()
                .iter()
                .enumerate()
                .all(|(from, &to)| universe[from].mv_type() == universe[to].mv_type())
        })
        .collect();
    Ok(Reduction {
        universe,
        indices,
        group,
        typed,
        id: NEXT_REDUCTION.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    })
}

impl Reduction {
    /// The model values the closure permutes; a map's images index into it.
    #[must_use]
    pub fn universe(&self) -> &[Atom] {
        &self.universe
    }

    /// The closure's nonidentity maps, in the order canonicalization tries
    /// them (the state itself is the initial candidate).
    #[must_use]
    pub fn maps(&self) -> &[crate::symmetry::Permutation] {
        self.group.maps()
    }

    /// Canonical values in the supplied variable order. The original values
    /// remain the explored state. TLC mode compares each candidate in TLC's
    /// argument order; canonical mode uses its approved total value order.
    ///
    /// # Errors
    /// Values outside the implemented pure permutation representations, value
    /// comparison errors, or the per-state permutation work/depth limit.
    pub fn canonical_values(
        &self,
        values: &[Value],
        variable_order: &[usize],
        ctx: Ctx,
    ) -> Result<Vec<Value>, BindError> {
        if variable_order.len() != values.len()
            || variable_order
                .iter()
                .enumerate()
                .any(|(at, index)| *index >= values.len() || variable_order[..at].contains(index))
        {
            return Err(BindError::bug(
                "symmetry variable order is not a permutation of the state slots",
            ));
        }
        let mut best = values.to_vec();
        let mut work = 0usize;
        // Reuse the two complete state rows, as TLC does. An early-abandoned
        // row is overwritten before it can become the new minimum.
        let mut candidate = values.to_vec();
        // Per slot: the elements of a set that may be compared without
        // building its image (see `distinct_elements`), computed once.
        let mut distinct: Vec<Option<Option<Distinct>>> = vec![None; values.len()];
        // Slots compared equal without building their image, in order.
        let mut unbuilt: Vec<usize> = Vec::new();
        // The map whose image is the best row (`None`: the state itself),
        // whether its images may be compared by rank, and per slot the rank
        // ids of the best row's set, computed when first needed.
        let mut best_from: Option<usize> = None;
        let mut best_ranked = true;
        let mut best_ids: Vec<Option<Option<Vec<u32>>>> = vec![None; values.len()];
        for (at, map) in self.group.maps().iter().enumerate() {
            let typed = self.typed.get(at).copied().unwrap_or(false);
            let mut smaller = false;
            unbuilt.clear();
            for &index in variable_order {
                let value = values
                    .get(index)
                    .ok_or_else(|| BindError::bug("symmetry variable order outside state"))?;
                if !smaller && typed {
                    if distinct[index].is_none() {
                        distinct[index] = Some(self.distinct(value, ctx));
                    }
                    // A map that fixes an unnormalized set yields the set
                    // itself, which the full comparison then normalizes.
                    if let Some(Some(elements)) = &distinct[index]
                        && !(elements.atoms.iter().all(|&i| map.image(i) == Some(i))
                            && matches!(value, Value::Set(set) if !set.is_tlc_normalized()))
                        && let Some(ordering) = self.compare_slot(
                            elements,
                            (at, map),
                            &best[index],
                            best_ranked
                                .then(|| {
                                    best_ids[index]
                                        .get_or_insert_with(|| Self::rank_ids(elements, best_from))
                                        .as_deref()
                                })
                                .flatten(),
                            ctx,
                            &mut work,
                        )?
                    {
                        match ordering {
                            Ordering::Greater => break,
                            Ordering::Equal => {
                                unbuilt.push(index);
                                continue;
                            }
                            Ordering::Less => {
                                unbuilt.push(index);
                                self.build_unbuilt(
                                    &unbuilt,
                                    true,
                                    values,
                                    map,
                                    (&mut candidate, &best),
                                    ctx,
                                    &mut work,
                                )?;
                                smaller = true;
                                continue;
                            }
                        }
                    }
                }
                candidate[index] = self.permute(value, map, ctx, 0, &mut work)?;
                if !smaller {
                    let ordering = compare_values(&candidate[index], &best[index], ctx)?;
                    match ordering {
                        Ordering::Greater => break,
                        Ordering::Less => {
                            self.build_unbuilt(
                                &unbuilt,
                                false,
                                values,
                                map,
                                (&mut candidate, &best),
                                ctx,
                                &mut work,
                            )?;
                            smaller = true;
                        }
                        Ordering::Equal => {}
                    }
                }
            }
            if smaller {
                std::mem::swap(&mut best, &mut candidate);
                best_from = Some(at);
                best_ranked = typed;
                best_ids.fill(None);
            }
        }
        Ok(best)
    }

    /// Compares a set slot's image under a map against the best row's slot
    /// without building the image: by rank when the best slot's rank ids
    /// are known, otherwise by value. `None` when `best` needs the full
    /// comparison. Under TLC's order, comparing normalizes `best`'s set,
    /// as `compareTo` does.
    fn compare_slot(
        &self,
        elements: &Distinct,
        (at, map): (usize, &Permutation),
        best: &Value,
        best_ids: Option<&[u32]>,
        ctx: Ctx,
        work: &mut usize,
    ) -> Result<Option<Ordering>, BindError> {
        *work = work.saturating_add(elements.elements.len());
        if *work > 32_000_000 {
            return Err(BindError::unsupported(
                "SYMMETRY per-state permutation work/depth limit",
            ));
        }
        if let Some(ids) = best_ids
            && let Some(ordering) = Self::compare_ranks(elements, at, ids)
        {
            if ctx.order == ValueOrder::Tlc
                && let Value::Set(set) = best
            {
                set.elements(ctx).map_err(|error| {
                    BindError::unsupported(format!(
                        "SYMMETRY candidate comparison: {}",
                        error.message()
                    ))
                })?;
            }
            return Ok(Some(ordering));
        }
        self.compare_image(elements, at, map, best, ctx, work)
    }

    /// Compares the image of a set with `elements` under the map at `at`
    /// against `best` without building it, or `None` when `best` needs the
    /// full comparison.
    fn compare_image(
        &self,
        elements: &Distinct,
        at: usize,
        map: &Permutation,
        best: &Value,
        ctx: Ctx,
        work: &mut usize,
    ) -> Result<Option<Ordering>, BindError> {
        let mut fresh = Vec::new();
        for (element, entry) in elements.elements.iter().zip(&elements.entries) {
            if entry.images.get(at).and_then(Option::as_ref).is_none() {
                fresh.push(self.permute(element, map, ctx, 1, work)?);
            }
        }
        let mut fresh = fresh.iter();
        let images = elements
            .entries
            .iter()
            .map(|entry| {
                entry
                    .images
                    .get(at)
                    .and_then(Option::as_ref)
                    .map(|(image, _)| image)
                    .or_else(|| fresh.next())
                    .ok_or_else(|| BindError::bug("SYMMETRY image cache lost an image"))
            })
            .collect::<Result<Vec<&Value>, _>>()?;
        if ctx.order == ValueOrder::Tlc {
            dive_value::tlc::compare_distinct_set(&images, best, ctx).map_err(|error| {
                BindError::unsupported(format!(
                    "SYMMETRY candidate comparison: {}",
                    error.message()
                ))
            })
        } else {
            Ok(Value::cmp_distinct_set(&images, best, ctx.order))
        }
    }

    /// The set slot's [`Distinct`] data, when its image may be compared
    /// without building it.
    fn distinct(&self, value: &Value, ctx: Ctx) -> Option<Distinct> {
        let elements = distinct_elements(value, ctx)?;
        let mut atoms = HashSet::new();
        for element in elements.iter() {
            self.collect_atoms(element, &mut atoms);
        }
        let mut atoms: Vec<usize> = atoms.into_iter().collect();
        atoms.sort_unstable();
        let (entries, generation) = self.cached_images(&elements, ctx);
        Some(Distinct {
            elements,
            entries,
            generation,
            atoms,
        })
    }

    /// The symmetric model values occurring in a set-free value.
    fn collect_atoms(&self, value: &Value, atoms: &mut HashSet<usize>) {
        match value {
            Value::Model(atom) => {
                if let Some(&index) = self.indices.get(atom) {
                    atoms.insert(index);
                }
            }
            Value::Fcn(function) => {
                if let Some(tuple) = function.as_tuple() {
                    for v in tuple {
                        self.collect_atoms(v, atoms);
                    }
                } else if let Some(fields) = function.as_record() {
                    for (_, v) in fields {
                        self.collect_atoms(v, atoms);
                    }
                } else {
                    for (domain, range) in function.iter() {
                        self.collect_atoms(&domain, atoms);
                        self.collect_atoms(range, atoms);
                    }
                }
            }
            _ => {}
        }
    }

    /// The cache entries of `elements`, from this thread's cache, and the
    /// generation that issued their rank ids.
    fn cached_images(&self, elements: &[Value], ctx: Ctx) -> (Vec<Arc<Entry>>, u64) {
        IMAGES.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.owner != Some((self.id, ctx)) {
                let generation = cache.generation + 1;
                *cache = ImageCache {
                    owner: Some((self.id, ctx)),
                    generation,
                    ..ImageCache::default()
                };
            }
            if cache
                .held
                .saturating_add(elements.len() * (self.group.maps().len() + 1))
                > IMAGE_CACHE_LIMIT
            {
                let generation = cache.generation + 1;
                *cache = ImageCache {
                    owner: cache.owner,
                    generation,
                    ..ImageCache::default()
                };
            }
            let entries = elements
                .iter()
                .map(|element| {
                    let key = Exact(element.clone());
                    if let Some(entry) = cache.images.get(&key) {
                        return entry.clone();
                    }
                    let images = self
                        .group
                        .maps()
                        .iter()
                        .map(|map| {
                            let image = self.permute(element, map, ctx, 1, &mut 0).ok()?;
                            let id = cache.rank_id(&image, ctx.order);
                            Some((image, id))
                        })
                        .collect();
                    let entry = Arc::new(Entry {
                        key: cache.rank_id(element, ctx.order),
                        images,
                    });
                    cache.held += self.group.maps().len() + 1;
                    cache.images.insert(key, entry.clone());
                    entry
                })
                .collect();
            (entries, cache.generation)
        })
    }

    /// The image of a set slot under the map at `at` against the best
    /// representative's slot, both given by rank ids, or `None` when either
    /// lacks them. `best` holds the best slot's rank ids in ascending order.
    fn compare_ranks(elements: &Distinct, at: usize, best: &[u32]) -> Option<Ordering> {
        thread_local! {
            static MINE: std::cell::RefCell<Vec<u64>> = const { std::cell::RefCell::new(Vec::new()) };
        }
        if elements.entries.len() != best.len() {
            return None;
        }
        IMAGES.with(|cache| {
            let cache = cache.borrow();
            if cache.generation != elements.generation {
                return None;
            }
            let rank = |id: u32| cache.rank.get(id as usize).copied();
            MINE.with(|mine| {
                let mut mine = mine.borrow_mut();
                mine.clear();
                for entry in &elements.entries {
                    mine.push(rank(entry.images.get(at)?.as_ref()?.1)?);
                }
                mine.sort_unstable();
                for (m, b) in mine.iter().zip(best) {
                    let c = m.cmp(&rank(*b)?);
                    if c != Ordering::Equal {
                        return Some(c);
                    }
                }
                Some(Ordering::Equal)
            })
        })
    }

    /// The rank ids of a set slot's image under the map at `at` (the
    /// elements themselves for `None`), in ascending rank order.
    fn rank_ids(elements: &Distinct, at: Option<usize>) -> Option<Vec<u32>> {
        let mut ids = elements
            .entries
            .iter()
            .map(|entry| match at {
                None => Some(entry.key),
                Some(at) => Some(entry.images.get(at)?.as_ref()?.1),
            })
            .collect::<Option<Vec<u32>>>()?;
        IMAGES.with(|cache| {
            let cache = cache.borrow();
            if cache.generation != elements.generation {
                return None;
            }
            let rank = |id: u32| cache.rank.get(id as usize).copied().unwrap_or(u64::MAX);
            ids.sort_unstable_by_key(|id| rank(*id));
            Some(ids)
        })
    }

    /// Builds the images of the slots that were compared without them, once
    /// the candidate is known to be smaller, and repeats their comparisons on
    /// the built values: the same comparisons, with the same normalizations,
    /// as if they had been built in the first place. When `last_less`, the
    /// last slot decided `Less`; every other slot compared equal.
    #[allow(
        clippy::too_many_arguments,
        reason = "the two state rows travel together with the map and work counter"
    )]
    fn build_unbuilt(
        &self,
        unbuilt: &[usize],
        last_less: bool,
        values: &[Value],
        map: &Permutation,
        (candidate, best): (&mut [Value], &[Value]),
        ctx: Ctx,
        work: &mut usize,
    ) -> Result<(), BindError> {
        for &index in unbuilt {
            let value = values
                .get(index)
                .ok_or_else(|| BindError::bug("symmetry variable order outside state"))?;
            candidate[index] = self.permute(value, map, ctx, 0, work)?;
            let ordering = compare_values(&candidate[index], &best[index], ctx)?;
            let expected = if last_less && Some(&index) == unbuilt.last() {
                Ordering::Less
            } else {
                Ordering::Equal
            };
            if ordering != expected {
                return Err(BindError::bug(
                    "SYMMETRY image comparison disagrees with the built image",
                ));
            }
        }
        Ok(())
    }

    /// TLC deep-normalizes the concrete source after selecting another
    /// representative, and before evaluating a VIEW even when identity wins.
    /// This preserves normalization observed by subsequent enumeration.
    ///
    /// # Errors
    /// A value normalization error or the same bounded work/depth refusal.
    pub fn normalize_concrete(values: &[Value], ctx: Ctx) -> Result<(), BindError> {
        let mut work = 0usize;
        for value in values {
            deep_normalize(value, ctx, 0, &mut work)?;
        }
        Ok(())
    }

    fn permute(
        &self,
        value: &Value,
        map: &Permutation,
        ctx: Ctx,
        depth: usize,
        work: &mut usize,
    ) -> Result<Value, BindError> {
        *work = work
            .checked_add(1)
            .ok_or_else(|| BindError::unsupported("SYMMETRY permutation work limit"))?;
        if *work > 32_000_000 || depth > 256 {
            return Err(BindError::unsupported(
                "SYMMETRY per-state permutation work/depth limit",
            ));
        }
        let recur =
            |value: &Value, work: &mut usize| self.permute(value, map, ctx, depth + 1, work);
        let map_error = |error: dive_value::ValueError| {
            BindError::unsupported(format!("SYMMETRY value permutation: {}", error.message()))
        };
        match value {
            Value::Model(atom) => Ok(self
                .indices
                .get(atom)
                .and_then(|&i| map.image(i))
                .and_then(|i| self.universe.get(i))
                .map_or_else(|| value.clone(), |atom| Value::Model(atom.clone()))),
            Value::Fcn(function) => permute_function(function, ctx, |value| recur(value, work)),
            Value::Set(set) => match set.kind() {
                SetKind::Enum | SetKind::Raw => {
                    let elements = match set.raw_elements() {
                        Some(elements) => Arc::from(elements),
                        None => set.elements(ctx).map_err(map_error)?,
                    };
                    if elements.len() > 32_000_000usize.saturating_sub(*work) {
                        return Err(BindError::unsupported(
                            "SYMMETRY set permutation work limit",
                        ));
                    }
                    let permuted = elements
                        .iter()
                        .map(|value| recur(value, work))
                        .collect::<Result<Vec<_>, _>>()?;
                    if elements
                        .iter()
                        .zip(&permuted)
                        .all(|(before, after)| same_value(before, after))
                    {
                        Ok(value.clone())
                    } else {
                        Ok(Value::set_raw(permuted, ctx))
                    }
                }
                SetKind::Interval => Ok(value.clone()),
                _ => Err(BindError::unsupported(format!(
                    "SYMMETRY permutation of lazy {:?} values",
                    set.kind()
                ))),
            },
            _ => Ok(value.clone()),
        }
    }
}

fn permute_function(
    function: &dive_value::Fcn,
    ctx: Ctx,
    mut recur: impl FnMut(&Value) -> Result<Value, BindError>,
) -> Result<Value, BindError> {
    let map_error = |error: dive_value::ValueError| {
        BindError::unsupported(format!("SYMMETRY value permutation: {}", error.message()))
    };
    if let Some(tuple) = function.as_tuple() {
        let values = tuple
            .iter()
            .map(&mut recur)
            .collect::<Result<Vec<_>, _>>()?;
        if tuple
            .iter()
            .zip(&values)
            .all(|(before, after)| same_value(before, after))
        {
            return Ok(Value::Fcn(function.clone()));
        }
        return Ok(if function.is_tuple_literal() {
            Value::tuple(values)
        } else {
            Value::tuple_fcn(values)
        });
    }
    if function.is_record_literal() {
        let fields = function
            .as_record()
            .ok_or_else(|| BindError::bug("record has no fields"))?
            .iter()
            .map(|(name, value)| Ok((name.clone(), recur(value)?)))
            .collect::<Result<Vec<_>, BindError>>()?;
        if function.as_record().is_some_and(|original| {
            original
                .iter()
                .zip(&fields)
                .all(|((_, before), (_, after))| same_value(before, after))
        }) {
            return Ok(Value::Fcn(function.clone()));
        }
        return Value::record(fields, ctx).map_err(map_error);
    }
    // FcnRcdValue.permute visits every range before any domain.
    let ranges = function
        .iter()
        .map(|(_, range)| recur(range))
        .collect::<Result<Vec<_>, BindError>>()?;
    let pairs = function
        .iter()
        .zip(ranges)
        .map(|((domain, _), range)| Ok((recur(&domain)?, range)))
        .collect::<Result<Vec<_>, BindError>>()?;
    if function
        .iter()
        .zip(&pairs)
        .all(|((domain, before), (image, after))| {
            same_value(&domain, image) && same_value(before, after)
        })
    {
        return Ok(Value::Fcn(function.clone()));
    }
    Value::fcn(pairs, ctx).map_err(map_error)
}

fn deep_normalize(
    value: &Value,
    ctx: Ctx,
    depth: usize,
    work: &mut usize,
) -> Result<(), BindError> {
    *work = work
        .checked_add(1)
        .ok_or_else(|| BindError::unsupported("SYMMETRY normalization work limit"))?;
    if *work > 32_000_000 || depth > 256 {
        return Err(BindError::unsupported(
            "SYMMETRY normalization work/depth limit",
        ));
    }
    match value {
        Value::Set(set) if matches!(set.kind(), SetKind::Enum | SetKind::Raw) => {
            // Children first, as SetEnumValue.deepNormalize. Reading raw values
            // here must not sort the parent before its children are normalized.
            if let Some(raw) = set.raw_elements() {
                for child in raw {
                    deep_normalize(child, ctx, depth + 1, work)?;
                }
            } else {
                for child in set
                    .elements(ctx)
                    .map_err(|e| {
                        BindError::unsupported(format!(
                            "SYMMETRY deep normalization: {}",
                            e.message()
                        ))
                    })?
                    .iter()
                {
                    deep_normalize(child, ctx, depth + 1, work)?;
                }
            }
            set.elements(ctx).map_err(|e| {
                BindError::unsupported(format!("SYMMETRY deep normalization: {}", e.message()))
            })?;
        }
        Value::Fcn(fcn) => {
            for (_, child) in fcn.iter() {
                deep_normalize(child, ctx, depth + 1, work)?;
            }
        }
        Value::Set(set) if set.as_deferred().is_some() => {
            let operand = set
                .as_deferred()
                .and_then(|provider| {
                    dive_eval::deferred_filter_normalization_operand(provider.as_ref())
                })
                .ok_or_else(|| {
                    BindError::unsupported(
                        "SYMMETRY deep normalization of unproved deferred values",
                    )
                })?;
            // SetPredValue visits only inVal, never its predicate. Unknown
            // providers and unproved lazy-operation caches remain refused
            // without invoking any provider method.
            deep_normalize(operand, ctx, depth + 1, work)?;
        }
        Value::Set(set) => {
            for (_, child) in set.components() {
                deep_normalize(&child, ctx, depth + 1, work)?;
            }
        }
        _ => {}
    }
    Ok(())
}

// TLC's permute methods retain the original object only when every child keeps
// identity. Extensional equality is insufficient: an invariant set may still
// have changed its raw element order, observable until normalization.
fn same_value(before: &Value, after: &Value) -> bool {
    match (before, after) {
        (Value::Set(a), Value::Set(b)) => a.ptr_eq(b),
        (Value::Fcn(a), Value::Fcn(b)) => a.ptr_eq(b),
        _ => before == after,
    }
}

/// A set slot whose image is compared without building it: its normalized
/// elements, each element's cache entry, and the cache generation that
/// issued their rank ids.
#[derive(Clone)]
struct Distinct {
    elements: Arc<[Value]>,
    entries: Vec<Arc<Entry>>,
    generation: u64,
    /// The symmetric model values occurring in the elements.
    atoms: Vec<usize>,
}

/// The candidate-against-best comparison: TLC's `compareTo` in TLC mode, the
/// approved total value order otherwise.
fn compare_values(candidate: &Value, best: &Value, ctx: Ctx) -> Result<Ordering, BindError> {
    if ctx.order == ValueOrder::Tlc {
        dive_value::tlc::compare(candidate, best, ctx).map_err(|error| {
            BindError::unsupported(format!(
                "SYMMETRY candidate comparison: {}",
                error.message()
            ))
        })
    } else {
        Ok(candidate.cmp_with(best, ctx.order))
    }
}

/// The normalized elements of `value` when it is an enumerated set whose
/// image can be compared without building it: no set occurs inside its
/// elements, so comparing them normalizes nothing, and the elements are
/// mutually comparable, so normalizing the image only sorts it. A map that
/// preserves model-value types keeps both properties and, being injective,
/// keeps the elements distinct. Reading the elements changes nothing TLC can
/// observe; the full comparison normalizes the same sets.
fn distinct_elements(value: &Value, ctx: Ctx) -> Option<Arc<[Value]>> {
    let Value::Set(set) = value else {
        return None;
    };
    if !matches!(set.kind(), SetKind::Enum | SetKind::Raw) {
        return None;
    }
    let elements = set.peek_elements(ctx).ok()?;
    let mut budget = 1_000_000usize;
    (elements
        .iter()
        .all(|element| set_free(element, 1, &mut budget))
        && Value::elements_comparable(&elements))
    .then_some(elements)
}

/// Whether no set occurs in `value`, within a work budget and the depth
/// limit of [`Reduction::permute`].
fn set_free(value: &Value, depth: usize, budget: &mut usize) -> bool {
    if *budget == 0 || depth > 256 {
        return false;
    }
    *budget -= 1;
    match value {
        Value::Set(_) => false,
        Value::Fcn(function) => {
            if let Some(tuple) = function.as_tuple() {
                tuple.iter().all(|v| set_free(v, depth + 1, budget))
            } else if let Some(fields) = function.as_record() {
                fields.iter().all(|(_, v)| set_free(v, depth + 1, budget))
            } else {
                function.iter().all(|(domain, range)| {
                    set_free(&domain, depth + 1, budget) && set_free(range, depth + 1, budget)
                })
            }
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test fixtures")]

    use super::*;
    use dive_value::Interner;

    /// The search before images were compared without building them: every
    /// candidate slot is permuted and compared in full.
    fn reference(
        reduction: &Reduction,
        values: &[Value],
        order: &[usize],
        ctx: Ctx,
    ) -> Result<Vec<Value>, BindError> {
        let mut best = values.to_vec();
        let mut work = 0usize;
        let mut candidate = values.to_vec();
        for map in reduction.group.maps() {
            let mut smaller = false;
            for &index in order {
                candidate[index] = reduction.permute(&values[index], map, ctx, 0, &mut work)?;
                if !smaller {
                    match compare_values(&candidate[index], &best[index], ctx)? {
                        Ordering::Greater => break,
                        Ordering::Less => smaller = true,
                        Ordering::Equal => {}
                    }
                }
            }
            if smaller {
                std::mem::swap(&mut best, &mut candidate);
            }
        }
        Ok(best)
    }

    struct Rng(u64);

    impl Rng {
        fn below(&mut self, n: u64) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0 % n
        }
    }

    fn reduction(names: &Interner, atoms: &[&str]) -> Reduction {
        let universe: Vec<Atom> = atoms.iter().map(|a| names.intern(a)).collect();
        let indices = universe
            .iter()
            .enumerate()
            .map(|(i, a)| (a.clone(), i))
            .collect();
        // Transpositions of neighbours generate every permutation.
        let generators: Vec<Vec<(usize, usize)>> = (0..universe.len() - 1)
            .map(|i| vec![(i, i + 1), (i + 1, i)])
            .collect();
        let group =
            Group::from_generators(universe.len(), &generators, Mode::Tlc, Limits::default())
                .unwrap();
        let typed = vec![true; group.maps().len()];
        Reduction {
            universe,
            indices,
            group,
            typed,
            id: NEXT_REDUCTION.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        }
    }

    /// A state with flat record sets of two shapes, tuple sets, a function
    /// keyed by model values, a scalar and a set of sets (the full path).
    fn state(names: &Interner, rng: &mut Rng, ctx: Ctx) -> Vec<Value> {
        let mv = |rng: &mut Rng| Value::model(names, &format!("m{}", rng.below(4)));
        let mut records = Vec::new();
        for _ in 0..rng.below(9) {
            let mut fields = vec![
                (names.intern("type"), Value::string(names, "a")),
                (
                    names.intern("b"),
                    Value::Int(i64::try_from(rng.below(3)).unwrap()),
                ),
                (names.intern("v"), mv(rng)),
            ];
            if rng.below(2) == 0 {
                fields[0].1 = Value::string(names, "b");
                fields.push((names.intern("acc"), mv(rng)));
            }
            records.push(Value::record(fields, ctx).unwrap());
        }
        let tuples = (0..rng.below(6))
            .map(|_| {
                Value::tuple(vec![
                    mv(rng),
                    Value::Int(i64::try_from(rng.below(2)).unwrap()),
                ])
            })
            .collect();
        let function = Value::fcn(
            (0..4)
                .map(|i| {
                    (
                        Value::model(names, &format!("m{i}")),
                        Value::Int(i64::try_from(rng.below(2)).unwrap()),
                    )
                })
                .collect(),
            ctx,
        )
        .unwrap();
        let nested = (0..rng.below(3))
            .map(|_| Value::set_raw(vec![mv(rng), mv(rng)], ctx))
            .collect();
        let functions = (0..rng.below(4))
            .map(|_| {
                let mut pairs = vec![(mv(rng), Value::Int(1))];
                let other = mv(rng);
                if pairs[0].0 != other {
                    pairs.push((other, Value::Int(2)));
                }
                Value::fcn(pairs, ctx).unwrap()
            })
            .collect();
        vec![
            Value::set_raw(records, ctx),
            Value::set_raw(functions, ctx),
            mv(rng),
            Value::set_raw(tuples, ctx),
            function,
            Value::set_raw(nested, ctx),
        ]
    }

    fn normalized(values: &[Value]) -> Vec<bool> {
        values
            .iter()
            .map(|v| matches!(v, Value::Set(s) if s.is_tlc_normalized()))
            .collect()
    }

    #[test]
    fn unbuilt_images_choose_the_reference_representative() {
        for ctx in [Ctx::tlc(), Ctx::default()] {
            let names = Interner::new();
            for a in ["m0", "m1", "m2", "m3", "type", "b", "v", "acc", "a"] {
                names.intern(a);
            }
            let reduction = reduction(&names, &["m0", "m1", "m2", "m3"]);
            for order in [
                [0usize, 1, 2, 3, 4, 5],
                [2, 4, 0, 5, 3, 1],
                [5, 3, 1, 0, 2, 4],
            ] {
                let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
                for _ in 0..300 {
                    let seed = rng.0;
                    let mine = state(&names, &mut Rng(seed), ctx);
                    let theirs = state(&names, &mut Rng(seed), ctx);
                    rng.below(2);
                    let got = reduction.canonical_values(&mine, &order, ctx).unwrap();
                    let want = reference(&reduction, &theirs, &order, ctx).unwrap();
                    assert_eq!(got, want, "order {order:?}, seed {seed}");
                    assert_eq!(
                        normalized(&mine),
                        normalized(&theirs),
                        "ctx {ctx:?} order {order:?} seed {seed} state {theirs:?} best {want:?}"
                    );
                    assert_eq!(
                        normalized(&got),
                        normalized(&want),
                        "ctx {ctx:?} order {order:?} seed {seed} got {got:?}"
                    );
                }
            }
        }
    }
}
