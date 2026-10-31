//! Parser-owned module causes in the demonstrated implied-action replay route.

use super::{OwnedTranscript, RunFacts, class, ec_name, exit_statuses_for, extract_counts};

/// E4's deciding module cause, only after the whole observed lifecycle matches.
#[allow(
    clippy::too_many_lines,
    reason = "one bounded immutable framing proof retains all phase and child checks"
)]
pub(super) fn cause(parsed: &OwnedTranscript, facts: &RunFacts<'_>) -> Option<i32> {
    let t = parsed.transcript();
    if facts.simulate
        || facts.trace_json.is_some()
        || !t.anomalies.is_empty()
        || t.messages().any(|message| {
            ec_name(message.code).is_none()
                || !matches!(
                    message.class,
                    class::NONE | class::ERROR | class::WARNING | class::STATE
                )
                || matches!(message.code, 2122 | 2193 | 2210 | 2218)
        })
        || t.messages()
            .filter(|message| message.class == class::ERROR)
            .map(|message| message.code)
            .collect::<Vec<_>>()
            != [2113, 2121, 2103]
    {
        return None;
    }
    let phases = [2185, 2189, 2190, 2113, 2121, 2103, 2199, 2194, 2186]
        .map(|code| parsed.unique(code))
        .into_iter()
        .collect::<Option<Vec<_>>>()?;
    for (position, &index) in phases.iter().enumerate() {
        let frame = parsed.frames().get(index)?;
        let message = parsed.message(index)?;
        let expected = if matches!(message.code, 2113 | 2121 | 2103) {
            class::ERROR
        } else {
            class::NONE
        };
        if frame.parent.is_some()
            || !frame.line_start
            || !frame.canonical
            || frame.end.is_none()
            || message.class != expected
            || (position > 0 && parsed.frames()[phases[position - 1]].end? > frame.start)
        {
            return None;
        }
    }
    let property = parsed
        .message(phases[3])?
        .body
        .strip_prefix("Evaluating action property ")?
        .strip_suffix(" failed.")?;
    if property.is_empty()
        || property.contains('\n')
        || !parsed.message(phases[0])?.body.starts_with("Starting... (")
        || !parsed.message(phases[0])?.body.ends_with(')')
        || parsed.message(phases[1])?.body != "Computing initial states..."
        || !parsed
            .message(phases[2])?
            .body
            .starts_with("Finished computing initial states: 1 distinct state generated at ")
        || parsed.message(phases[4])?.body != "The behavior up to this point is:"
        || !super::replay_stack(&parsed.message(phases[5])?.body)
        || !parsed.message(phases[8])?.body.starts_with("Finished in ")
        || !parsed.message(phases[8])?.body.ends_with(')')
    {
        return None;
    }
    let nested: Vec<_> = parsed
        .frames()
        .iter()
        .enumerate()
        .filter(|(_, frame)| frame.parent.is_some())
        .collect();
    let [(index, frame)] = nested.as_slice() else {
        return None;
    };
    let message = parsed.message(*index)?;
    if frame.parent != Some(phases[3])
        || !frame.line_start
        || !frame.canonical
        || frame.end.is_none()
        || message.class != class::NONE
        || message.body.is_empty()
        || frame.parent_body_prefix_len != parsed.frames()[phases[3]].own_body_len
        || !matches!(message.code, 2132 | 2179 | 2178)
        || parsed.unique(message.code)? != *index
        || !exit_statuses_for(message.code).contains(&facts.exit_code?)
    {
        return None;
    }
    let mut count = 0_u64;
    for (index, frame) in parsed.frames().iter().enumerate() {
        let state = parsed.message(index)?;
        if state.code != 2217 {
            continue;
        }
        let (number, label, values) = super::split_state(&state.body);
        count += 1;
        if frame.parent.is_some()
            || !frame.line_start
            || !frame.canonical
            || frame.start < parsed.frames()[phases[4]].end?
            || frame.end? > parsed.frames()[phases[5]].start
            || state.class != class::STATE
            || number != Some(count)
            || values.trim().is_empty()
            || count == 1 && label != "<Initial predicate>"
        {
            return None;
        }
    }
    let counts = extract_counts(t);
    counts.distinct_states?;
    counts.states_generated?;
    counts.states_left_on_queue?;
    counts.depth?;
    (count > 1).then_some(message.code)
}
