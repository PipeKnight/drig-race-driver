//! One serial owner callback followed by conditional response consumption.
//! Contacts, owner registration, pending lifetime and scheduling are supplied.
use crate::{Error, collision_callback, response_consumption as consumer};

pub struct Input<'a> {
    pub callback: collision_callback::Input<'a>,
    pub owner: u32,
    pub contacts: &'a [consumer::Contact],
    pub selected: &'a [usize],
    pub pending: &'a [consumer::Pending],
}

pub struct Output {
    pub callback: collision_callback::Output,
    /// Absent when the callback guard declines the call. Supplied contact and
    /// pending state remain unchanged in that case.
    pub consumption: Option<consumer::Output>,
}

pub fn calculate(input: Input<'_>) -> Result<Output, Error> {
    let generated = input.callback.producer.generation.contacts;
    if generated.len() > 80 {
        return Err(Error("serial callback contact count exceeds bound"));
    }
    let zeros = vec![0.; generated.len()];
    let operands = consumer::Input {
        owner: input.owner,
        contacts: input.contacts,
        selected: input.selected,
        response: &zeros,
        pending: input.pending,
    };
    consumer::validate(&operands)?;
    if generated.len() != input.selected.len()
        || generated.iter().zip(input.selected).any(|(g, &index)| {
            let c = &input.contacts[index];
            let current = if g.reverse {
                c.second.filter(|_| c.second_is_body)
            } else {
                c.first
            };
            c.accumulated.to_bits() != g.value_38.to_bits()
                || !current.is_some_and(|o| o.identity == input.owner && o.registered)
        })
    {
        return Err(Error("serial callback contact correspondence mismatch"));
    }
    // The native serial wrapper supplies an initialized response array even if
    // its caller would otherwise have supplied values or a null response.
    let callback = collision_callback::calculate(collision_callback::Input {
        producer: crate::impact_producer::Input {
            response: crate::impact_producer::Response {
                values: Some(&zeros),
                ..input.callback.producer.response
            },
            ..input.callback.producer
        },
        ..input.callback
    })?;
    let consumption = if let Some(producer) = &callback.producer {
        let values = producer
            .response
            .response
            .as_deref()
            .ok_or(Error("serial callback omitted its response array"))?;
        Some(consumer::consume(consumer::Input {
            response: values,
            ..operands
        })?)
    } else {
        None
    };
    Ok(Output {
        callback,
        consumption,
    })
}
