// Inserted only into the study's private Core source copy.
for check in invariants.iter().chain(&implied_actions) {
    c.code.operator_roots.extend(check.conjuncts.iter().map(|part|(part.p,u32::MAX)));
}
for node in &c.code.gnodes {
    if let GNode::Guard(p) = node { c.code.operator_roots.push((*p,u32::MAX-1)); }
}
if !next.is_empty()
    && let Some(output) = std::env::var_os("DIVE_EXPORT_SUCCESSORS")
{
    if mapped || c.effects.any {
        return Err(Error::unsupported("native successor study requires a pure unmapped action plan"));
    }
    use std::io::Write;
    let mut file = std::fs::File::create_new(output)
        .map_err(|error| Error::internal(format!("could not create the requested plan: {error}")))?;
    for shape in crate::eval::operators::Operators::shapes(&c.code) {
        writeln!(file,"operator {shape}").map_err(|e| Error::internal(format!("could not export shared operator: {e}")))?;
    }
    for action in &next {
        writeln!(file, "root {}", action.g)
            .map_err(|error| Error::internal(format!("could not export the action: {error}")))?;
    }
    for node in &c.code.gnodes {
        writeln!(file, "node {node:?}")
            .map_err(|error| Error::internal(format!("could not export the generator: {error}")))?;
    }
    return Err(Error::unsupported("native successor study exported the plan before exploration"));
}
c.code.native_worker = !mapped && !c.effects.any && crate::generate::native_study::matches(&c.code);
if c.code.native_worker && std::env::var_os("DIVE_DISABLE_NATIVE_BLOCKS").is_none() {
    c.code.native_blocks = crate::native_blocks::Blocks::bind(&c.code,&c.m.heap);
}
if c.code.native_worker && std::env::var_os("DIVE_DISABLE_NATIVE_OPERATORS").is_none() {
    c.code.operators = crate::eval::operators::Operators::bind(&c.code);
}
