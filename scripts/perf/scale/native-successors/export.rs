// Inserted only into the study's private Core source copy.
if !next.is_empty()
    && let Some(output) = std::env::var_os("DIVE_EXPORT_SUCCESSORS")
{
    if mapped || c.effects.any {
        return Err(Error::unsupported("native successor study requires a pure unmapped action plan"));
    }
    use std::io::Write;
    let mut file = std::fs::File::create_new(output)
        .map_err(|error| Error::internal(format!("could not create the requested plan: {error}")))?;
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
