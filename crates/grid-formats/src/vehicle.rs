//! Supported Standard tuning prelude, verified against original live body fields.
//! Standard wheel geometry follows the named .ned inputs and original offset chain.
use crate::{Result, binxml::Document, fail, tuning::Row};

/// First entry is the early CSV copy to wheel +0xe0; remaining entries are scalar copies.
/// The geometric initializer subsequently replaces +0xe0 with the .ned radius.
/// Their force semantics have not been reconstructed.
pub const WHEEL_GROUP_COLUMNS: [[usize; 8]; 2] = [
    [146, 84, 86, 88, 90, 96, 94, 80],
    [145, 83, 85, 87, 89, 95, 93, 79],
];

#[derive(Debug, Clone, Copy)]
pub struct WheelParameterGroup {
    /// Early CSV value; use StandardWheelLayout::radii for initialized clearance.
    pub clearance_operand: f32,
    pub direct_scalars: [f32; 7],
}

#[derive(Debug, Clone, Copy)]
pub struct StandardBodyParameters {
    pub mass: f32,
    pub inverse_mass: f32,
    pub inertia_components: [f32; 3],
    /// Group zero initializes wheel ordinals 0/1; group one initializes 2/3.
    pub wheel_groups: [WheelParameterGroup; 2],
}

/// Select the extra coupled tyre axes used by the original wheel substep.
/// This is independent of the downstream production tyre force computation.
/// The original decoder compares CSV column 311 with 0.5 before f32 storage.
pub fn read_standard_coupled_tyres(row: &Row<'_>) -> Result<bool> {
    read_standard_body(row)?;
    Ok(row.number(311)?.unwrap_or(0.) > 0.5)
}

/// Original properties +53c/+540 copied to body +1fa4/+1fa8.
/// These reductions select positive/nonpositive wheel-record Z respectively.
/// Zero defaults apply after f64 parsing and f32 storage, including underflow.
pub fn read_standard_grip_reductions(row: &Row<'_>) -> Result<[f32; 2]> {
    read_standard_body(row)?;
    let number = |column| -> Result<f32> {
        let value = row.number(column)?.unwrap_or(0.) as f32;
        if !value.is_finite() {
            return fail(column, "grip reduction exceeds finite f32 range");
        }
        Ok(if value == 0. { 0.5 } else { value })
    };
    Ok([number(178)?, number(179)?])
}

/// CSV column77 through original properties+340 to steering body+e54.
/// Multiply parsed f64 by the stored-f32 degree conversion before storing f32.
/// There is no zero substitution in the inspected decoder.
pub fn read_standard_steering_angle(row: &Row<'_>) -> Result<f32> {
    read_standard_body(row)?;
    let degrees = row.number(77)?.unwrap_or(0.);
    let angle = (degrees * f64::from(f32::from_bits(0x3c8e_fa35))) as f32;
    if !angle.is_finite() {
        return fail(77, "steering angle exceeds finite f32 range");
    }
    Ok(angle)
}

/// Initial Standard suspension fields. Runtime may subsequently change them;
/// in particular the observed running damping threshold differs from the CSV.
#[derive(Debug, Clone, Copy)]
pub struct SuspensionInitialization {
    pub spring: f32,
    pub damper: f32,
    pub axle_coupling: f32,
    pub bump: f32,
    pub speed_threshold: f32,
    pub high_speed_ratio: f32,
    pub negative_speed_scale: f32,
    pub travel_denominator: f32,
    pub preload: f32,
    pub lower_stop: f32,
    pub upper_stop: f32,
}

/// Return rear/front initialization groups in the verified Standard wheel order.
/// Empty optional coefficient cells become zero; no arbitrary missing-row padding.
pub fn read_standard_suspension(row: &Row<'_>) -> Result<[SuspensionInitialization; 2]> {
    let body = read_standard_body(row)?;
    let number = |column| -> Result<f32> {
        let value = row.number(column)?.unwrap_or(0.) as f32;
        if !value.is_finite() {
            return fail(column, "suspension coefficient exceeds finite f32 range");
        }
        Ok(value)
    };
    let rear_negative = number(116)?;
    let rear_negative = if rear_negative == 0. {
        1.
    } else {
        rear_negative
    };
    let front_negative = number(275)?;
    let front_negative = if front_negative == 0. {
        rear_negative
    } else {
        front_negative
    };
    let group = |index: usize,
                 bump_column,
                 threshold_column,
                 ratio_column,
                 negative|
     -> Result<SuspensionInitialization> {
        let scalars = body.wheel_groups[index].direct_scalars;
        Ok(SuspensionInitialization {
            spring: scalars[4],
            damper: scalars[5],
            axle_coupling: scalars[6],
            bump: number(bump_column)?,
            speed_threshold: number(threshold_column)?,
            high_speed_ratio: number(ratio_column)?,
            negative_speed_scale: negative,
            travel_denominator: scalars[2].max(f32::from_bits(0x3e99_999a)),
            preload: 0.,
            lower_stop: scalars[3],
            upper_stop: scalars[2],
        })
    };
    Ok([
        group(0, 130, 213, 215, rear_negative)?,
        group(1, 137, 212, 214, front_negative)?,
    ])
}

/// Suspension subset of thirty setup entries, indexed as the original slider
/// array: spring 4/5, damper 6/8, negative damper 7/9, high-speed force 10,
/// threshold 11, height 12, axle coupling 28/29. Other entries remain absent.
#[derive(Debug, Clone, Copy)]
pub struct SuspensionSetup {
    pub ranges: [Option<[f32; 2]>; 30],
    pub sliders: [Option<f32>; 30],
    pub base_damper: [f32; 2],
    pub base_threshold: [f32; 2],
    pub base_high_ratio: [f32; 2],
    pub height: f32,
    pub explicit_ranges: bool,
}

/// Read the suspension subset of custom/fallback ranges and default sliders.
/// Source conversion/defaults are distinct from the full original CSV decoder.
pub fn read_standard_suspension_setup(row: &Row<'_>) -> Result<SuspensionSetup> {
    let groups = read_standard_suspension(row)?;
    let number = |column| -> Result<f32> {
        let value = row.number(column)?.unwrap_or(0.) as f32;
        if !value.is_finite() {
            return fail(column, "setup value exceeds finite f32 range");
        }
        Ok(value)
    };
    let explicit = (216..254)
        .chain(264..268)
        .all(|i| !row.fields[i].is_empty());
    let front = groups[1];
    let rear = groups[0];
    let damper = [front.damper, rear.damper];
    let threshold = [front.speed_threshold, rear.speed_threshold];
    let ratio = [front.high_speed_ratio, rear.high_speed_ratio];
    let high_force = [ratio[0] * damper[0], damper[1] * ratio[1]];
    let height = number(194)?;
    let defaults = [
        (4, front.spring),
        (5, rear.spring),
        (6, damper[0]),
        (7, front.negative_speed_scale * damper[0]),
        (8, damper[1]),
        (9, damper[1] * rear.negative_speed_scale),
        (10, (high_force[0] + high_force[1]) * 0.5),
        (11, (threshold[0] + threshold[1]) * 0.5),
        (12, height),
        (28, front.axle_coupling),
        (29, rear.axle_coupling),
    ];
    let custom_columns = [
        (4, 228),
        (5, 230),
        (6, 232),
        (7, 236),
        (8, 234),
        (9, 238),
        (10, 240),
        (11, 242),
        (12, 224),
        (28, 246),
        (29, 248),
    ];
    let mut ranges = [None; 30];
    let mut sliders = [None; 30];
    let lower_scale = f32::from_bits(0x3f4c_cccd);
    let upper_scale = f32::from_bits(0x3f99_999a);
    for ((index, base), (_, column)) in defaults.into_iter().zip(custom_columns) {
        let range = if explicit {
            [number(column)?, number(column + 1)?]
        } else if index == 12 {
            [height - 0.5, height + 0.5]
        } else if index == 10 {
            [
                high_force[0].min(high_force[1]) * lower_scale,
                high_force[0].max(high_force[1]) * upper_scale,
            ]
        } else if index == 11 {
            [
                threshold[0].min(threshold[1]) * lower_scale,
                threshold[0].max(threshold[1]) * upper_scale,
            ]
        } else {
            [base * lower_scale, base * upper_scale]
        };
        if !base.is_finite() || range.iter().any(|v| !v.is_finite()) {
            return fail(column, "setup range exceeds finite f32 range");
        }
        let slider = if range[0] < range[1] {
            let clamped = base.max(range[0]).min(range[1]);
            (clamped - range[0]) / (range[1] - range[0])
        } else {
            0.
        };
        ranges[index] = Some(range);
        sliders[index] = Some(slider);
    }
    Ok(SuspensionSetup {
        ranges,
        sliders,
        base_damper: damper,
        base_threshold: threshold,
        base_high_ratio: ratio,
        height,
        explicit_ranges: explicit,
    })
}

fn required_f32(row: &Row<'_>, column: usize) -> Result<f32> {
    let value = row.number(column)?.ok_or(crate::Error {
        offset: column,
        message: "required vehicle numeric field is empty",
    })? as f32;
    if !value.is_finite() {
        return fail(column, "vehicle value exceeds finite f32 range");
    }
    Ok(value)
}

#[derive(Debug, Clone, Copy)]
pub struct AxleGeometry {
    /// Positive-X side; the opposite side is mirrored about local X=0.
    pub position: [f32; 3],
    pub radius: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct WheelGeometry {
    pub rear: AxleGeometry,
    pub front: AxleGeometry,
}

#[derive(Debug, Clone, Copy)]
pub struct StandardWheelLayout {
    /// Original additive local-frame offset; homogeneous component zero.
    pub offset: [f32; 4],
    /// Rear positive-X, rear negative-X, front positive-X, front negative-X.
    pub positions: [[f32; 4]; 4],
    /// .ned radii, copied to both initialized wheel +0xe0 and +0xf4.
    /// These supersede the early CSV clearance operands.
    pub radii: [f32; 4],
}

pub fn read_wheel_geometry(document: &Document<'_>) -> Result<WheelGeometry> {
    let mut matches = document.elements.iter().filter(|e| e.name == "WHEELS");
    let element = matches.next().ok_or(crate::Error {
        offset: 0,
        message: "car WHEELS element is absent",
    })?;
    if matches.next().is_some() {
        return fail(0, "car WHEELS element is ambiguous");
    }
    let number = |name: &str| -> Result<f32> {
        let mut values = element.attributes.iter().filter(|(key, _)| *key == name);
        let text = values
            .next()
            .ok_or(crate::Error {
                offset: 0,
                message: "required wheel geometry attribute is absent",
            })?
            .1;
        if values.next().is_some() {
            return fail(0, "duplicate wheel geometry attribute");
        }
        let value = text.parse::<f64>().map_err(|_| crate::Error {
            offset: 0,
            message: "wheel geometry attribute is not numeric",
        })? as f32;
        if !value.is_finite() {
            return fail(0, "nonfinite wheel geometry attribute");
        }
        Ok(value)
    };
    let rear = AxleGeometry {
        position: [
            number("m_rear_x")?,
            number("m_rear_y")?,
            number("m_rear_z")?,
        ],
        radius: number("m_rear_r")?,
    };
    let front = AxleGeometry {
        position: [
            number("m_front_x")?,
            number("m_front_y")?,
            number("m_front_z")?,
        ],
        radius: number("m_front_r")?,
    };
    if rear.radius <= 0.0 || front.radius <= 0.0 {
        return fail(0, "wheel geometry radius must be positive");
    }
    Ok(WheelGeometry { rear, front })
}

/// Reconstruct the supported Standard geometric initialization; no forces/contact.
pub fn standard_wheel_layout(
    row: &Row<'_>,
    geometry: WheelGeometry,
) -> Result<StandardWheelLayout> {
    read_standard_body(row)?;
    let height_fraction = required_f32(row, 7)?;
    let longitudinal_fraction = required_f32(row, 8)?;
    let vertical_bias = required_f32(row, 194)?;
    let rear = geometry.rear;
    let front = geometry.front;
    if rear
        .position
        .iter()
        .chain(&front.position)
        .any(|v| !v.is_finite())
        || !rear.radius.is_finite()
        || !front.radius.is_finite()
        || rear.radius <= 0.0
        || front.radius <= 0.0
    {
        return fail(0, "invalid wheel geometry input");
    }
    let mut positions = [
        [
            rear.position[0],
            rear.position[1] - vertical_bias,
            rear.position[2],
            1.0,
        ],
        [
            -0.0 - rear.position[0],
            rear.position[1] - vertical_bias,
            rear.position[2],
            1.0,
        ],
        [
            front.position[0],
            front.position[1] - vertical_bias,
            front.position[2],
            1.0,
        ],
        [
            -0.0 - front.position[0],
            front.position[1] - vertical_bias,
            front.position[2],
            1.0,
        ],
    ];
    let radii = [rear.radius, rear.radius, front.radius, front.radius];
    let span = ((positions[0][0] - positions[1][0]) + (positions[2][0] - positions[3][0])) * 0.5;
    let height = span * height_fraction + vertical_bias;
    let bottom_average = (((positions[2][1] - radii[2]) + (positions[1][1] - radii[1]))
        + (positions[3][1] - radii[3])
        + (positions[0][1] - radii[0]))
        * 0.25;
    let rear_z = (positions[0][2] + positions[1][2]) * 0.5;
    let front_z = (positions[2][2] + positions[3][2]) * 0.5;
    let offset = [
        -0.0 - (((positions[0][0] + positions[1][0]) + positions[2][0]) + positions[3][0]) * 0.25,
        (-0.0 - height) - bottom_average,
        -0.0 - ((front_z - rear_z) * longitudinal_fraction + rear_z),
        0.0,
    ];
    for position in &mut positions {
        for axis in 0..3 {
            position[axis] += offset[axis];
        }
    }
    if offset
        .iter()
        .chain(positions.iter().flatten())
        .any(|v| !v.is_finite())
    {
        return fail(0, "wheel layout calculation overflowed");
    }
    Ok(StandardWheelLayout {
        offset,
        positions,
        radii,
    })
}

pub fn read_standard_body(row: &Row<'_>) -> Result<StandardBodyParameters> {
    if !(row.fields.len() == 360 || row.fields.len() == 361 && row.fields[360].is_empty()) {
        return fail(0, "vehicle prelude requires 360 consumed fields");
    }
    if row.fields[2] != "Standard" {
        return fail(2, "unsupported vehicle wheel arrangement");
    }
    let mass = required_f32(row, 3)?;
    let inverse_mass = 1.0 / mass;
    if mass <= 0.0 || !inverse_mass.is_finite() || inverse_mass <= 0.0 {
        return fail(3, "vehicle mass or its f32 reciprocal is invalid");
    }
    let inertia_components = [
        required_f32(row, 4)?,
        required_f32(row, 5)?,
        required_f32(row, 6)?,
    ];
    let group = |columns: [usize; 8]| -> Result<WheelParameterGroup> {
        Ok(WheelParameterGroup {
            clearance_operand: required_f32(row, columns[0])?,
            direct_scalars: [
                required_f32(row, columns[1])?,
                required_f32(row, columns[2])?,
                required_f32(row, columns[3])?,
                required_f32(row, columns[4])?,
                required_f32(row, columns[5])?,
                required_f32(row, columns[6])?,
                required_f32(row, columns[7])?,
            ],
        })
    };
    Ok(StandardBodyParameters {
        mass,
        inverse_mass,
        inertia_components,
        wheel_groups: [
            group(WHEEL_GROUP_COLUMNS[0])?,
            group(WHEEL_GROUP_COLUMNS[1])?,
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<&'static str> {
        let mut fields = vec!["1"; 360];
        fields[2] = "Standard";
        fields
    }

    #[test]
    fn grip_reduction_defaults_follow_f32_storage_including_underflow() {
        let mut fields = fixture();
        for text in ["", "0", "-0", "1e-46", "-1e-46"] {
            fields[178] = text;
            fields[179] = text;
            assert_eq!(
                read_standard_grip_reductions(&Row {
                    fields: fields.clone()
                })
                .unwrap(),
                [0.5; 2]
            );
        }
        fields[178] = "1e-45";
        fields[179] = "-1e-45";
        let values = read_standard_grip_reductions(&Row {
            fields: fields.clone(),
        })
        .unwrap();
        assert_eq!(values[0].to_bits(), 1);
        assert_eq!(values[1].to_bits(), 0x8000_0001);
        for text in ["NaN", "inf", "1e300"] {
            fields[178] = text;
            assert!(
                read_standard_grip_reductions(&Row {
                    fields: fields.clone()
                })
                .is_err()
            );
        }
    }

    #[test]
    fn coupled_tyre_selection_preserves_precision_at_the_activation_boundary() {
        let mut fields = fixture();
        for (value, expected) in [
            ("", false),
            ("0.5", false),
            ("0.5000000000000001", true),
            ("0.49999999999999994", false),
        ] {
            fields[311] = value;
            assert_eq!(
                read_standard_coupled_tyres(&Row {
                    fields: fields.clone()
                })
                .unwrap(),
                expected
            );
        }
        fields[311] = "NaN";
        assert!(read_standard_coupled_tyres(&Row { fields }).is_err());
    }

    #[test]
    fn suspension_initialization_handles_defaults_clamp_and_invalid_optional_fields() {
        let mut fields = fixture();
        fields[116] = "";
        fields[275] = "0";
        fields[130] = "";
        fields[88] = "0.1";
        fields[87] = "0.6";
        fields[213] = "400";
        fields[212] = "500";
        fields[215] = "0.35";
        fields[214] = "0.4";
        let result = read_standard_suspension(&Row {
            fields: fields.clone(),
        })
        .unwrap();
        assert_eq!(result[0].negative_speed_scale, 1.);
        assert_eq!(result[1].negative_speed_scale, 1.);
        assert_eq!(result[0].travel_denominator, 0.3);
        assert_eq!(result[0].upper_stop, 0.1);
        assert_eq!(result[1].travel_denominator, 0.6);
        assert_eq!(result[0].bump, 0.);
        assert_eq!(result[0].speed_threshold, 400.);
        assert_eq!(result[1].speed_threshold, 500.);
        fields[116] = "0.8";
        let result = read_standard_suspension(&Row {
            fields: fields.clone(),
        })
        .unwrap();
        assert_eq!(result[1].negative_speed_scale, 0.8);
        for invalid in ["NaN", "1e300", "bad"] {
            fields[130] = invalid;
            assert!(
                read_standard_suspension(&Row {
                    fields: fields.clone()
                })
                .is_err()
            );
        }
    }

    #[test]
    fn suspension_setup_clamps_explicit_threshold_and_builds_fallback() {
        let mut fields = fixture();
        for field in &mut fields[216..254] {
            *field = "0";
        }
        for field in &mut fields[264..268] {
            *field = "0";
        }
        fields[212] = "400";
        fields[213] = "400";
        fields[242] = "0.001";
        fields[243] = "0.14";
        let explicit = read_standard_suspension_setup(&Row {
            fields: fields.clone(),
        })
        .unwrap();
        assert!(explicit.explicit_ranges);
        assert_eq!(explicit.ranges[11], Some([0.001, 0.14]));
        assert_eq!(explicit.sliders[11], Some(1.));
        assert!(explicit.ranges[0].is_none());
        fields[216] = "";
        let fallback = read_standard_suspension_setup(&Row {
            fields: fields.clone(),
        })
        .unwrap();
        assert!(!fallback.explicit_ranges);
        assert_eq!(fallback.ranges[11], Some([320., 480.00003]));
        fields[243] = "NaN";
        fields[216] = "0";
        assert!(read_standard_suspension_setup(&Row { fields }).is_err());
    }

    #[test]
    fn geometric_layout_preserves_axle_order_mirroring_and_vertical_bias() {
        let mut fields = fixture();
        fields[7] = "0.25";
        fields[8] = "0.5";
        fields[194] = "0.125";
        let geometry = WheelGeometry {
            rear: AxleGeometry {
                position: [1., 1., -2.],
                radius: 0.5,
            },
            front: AxleGeometry {
                position: [2., 1., 2.],
                radius: 0.5,
            },
        };
        let row = Row { fields };
        let prelude = read_standard_body(&row).unwrap();
        assert_eq!(prelude.wheel_groups[0].clearance_operand, 1.0);
        assert_eq!(prelude.wheel_groups[1].clearance_operand, 1.0);
        let layout = standard_wheel_layout(&row, geometry).unwrap();
        assert_eq!(layout.offset, [-0., -1.25, -0., 0.]);
        assert_eq!(layout.offset[0].to_bits(), (-0.0_f32).to_bits());
        assert_eq!(
            layout.positions,
            [
                [1., -0.375, -2., 1.],
                [-1., -0.375, -2., 1.],
                [2., -0.375, 2., 1.],
                [-2., -0.375, 2., 1.],
            ]
        );
        assert_eq!(layout.radii, [0.5; 4]);
    }

    #[test]
    fn geometry_rejects_missing_ambiguous_nonfinite_or_invalid_radius_inputs() {
        use crate::binxml::Element;
        let attributes = vec![
            ("m_rear_x", "1"),
            ("m_rear_y", "1"),
            ("m_rear_z", "-2"),
            ("m_rear_r", "0.5"),
            ("m_front_x", "1"),
            ("m_front_y", "1"),
            ("m_front_z", "2"),
            ("m_front_r", "0.5"),
        ];
        let document = |attributes| Document {
            strings: vec![],
            string_padding: &[],
            elements: vec![Element {
                name: "WHEELS",
                value: None,
                attributes,
                children: 1..1,
                parent: None,
            }],
        };
        assert!(read_wheel_geometry(&document(attributes.clone())).is_ok());
        let mut missing = attributes.clone();
        missing.pop();
        assert!(read_wheel_geometry(&document(missing)).is_err());
        let mut duplicate = attributes.clone();
        duplicate.push(("m_rear_x", "2"));
        assert!(read_wheel_geometry(&document(duplicate)).is_err());
        for invalid in ["NaN", "inf", "1e300", "0", "-1"] {
            let mut bad = attributes.clone();
            bad[3].1 = invalid;
            assert!(read_wheel_geometry(&document(bad)).is_err());
        }
        let mut ambiguous = document(attributes.clone());
        ambiguous
            .elements
            .push(document(attributes).elements.remove(0));
        assert!(read_wheel_geometry(&ambiguous).is_err());
        let geometry = WheelGeometry {
            rear: AxleGeometry {
                position: [f32::MAX, 0., 0.],
                radius: 1.,
            },
            front: AxleGeometry {
                position: [f32::MAX, 0., 0.],
                radius: 1.,
            },
        };
        assert!(standard_wheel_layout(&Row { fields: fixture() }, geometry).is_err());
    }

    #[test]
    fn rejects_f32_overflow_empty_fields_and_singular_reciprocals() {
        for mass in ["", "0", "-1", "1e-46", "1e-40", "1e300"] {
            let mut fields = fixture();
            fields[3] = mass;
            assert!(read_standard_body(&Row { fields }).is_err());
        }
        let mut fields = fixture();
        fields[146] = "1e300";
        assert!(read_standard_body(&Row { fields }).is_err());
    }

    #[test]
    fn scoped_prelude_accepts_only_supported_arrangement_and_consumed_shape() {
        let fields = fixture();
        assert!(
            read_standard_body(&Row {
                fields: fields.clone()
            })
            .is_ok()
        );
        let mut trailing = fields.clone();
        trailing.push("");
        assert!(read_standard_body(&Row { fields: trailing }).is_ok());
        let mut extra = fields.clone();
        extra.push("2");
        assert!(read_standard_body(&Row { fields: extra }).is_err());
        let mut arrangement = fields.clone();
        arrangement[2] = "6 Wheeler";
        assert!(
            read_standard_body(&Row {
                fields: arrangement
            })
            .is_err()
        );
        assert!(
            read_standard_body(&Row {
                fields: fields[..146].to_vec()
            })
            .is_err()
        );
    }
}
