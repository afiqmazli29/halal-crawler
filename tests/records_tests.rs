use halal_crawler::records::Company;

// ── Company::fill_from ─────────────────────────────────────────

fn listing_co() -> Company {
    Company {
        name: "ABC Sdn Bhd".to_string(),
        address: "1 Jalan, 50000 KL".to_string(),
        postcode: "50000".to_string(),
        state: "Kuala Lumpur".to_string(),
        comp_code: "COMP-1".to_string(),
        ..Default::default()
    }
}

#[test]
fn test_fill_from_fills_empty_fields() {
    let mut modal = Company {
        phone_no: "03-1".to_string(),
        ..Default::default()
    };
    modal.fill_from(&listing_co());

    assert_eq!(modal.address, "1 Jalan, 50000 KL");
    assert_eq!(modal.postcode, "50000");
    assert_eq!(modal.state, "Kuala Lumpur");
    assert_eq!(modal.comp_code, "COMP-1");
    // The modal's own value is preserved.
    assert_eq!(modal.phone_no, "03-1");
}

#[test]
fn test_fill_from_keeps_modal_values() {
    let mut modal = Company {
        address: "Modal Address".to_string(),
        postcode: "99999".to_string(),
        state: "Selangor".to_string(),
        comp_code: "COMP-2".to_string(),
        ..Default::default()
    };
    modal.fill_from(&listing_co());

    assert_eq!(modal.address, "Modal Address");
    assert_eq!(modal.postcode, "99999");
    assert_eq!(modal.state, "Selangor");
    assert_eq!(modal.comp_code, "COMP-2");
}

#[test]
fn test_fill_from_never_takes_name() {
    // The listing name is identity; the modal name is ignored entirely.
    let mut modal = Company {
        name: "ABC SDN. BHD.".to_string(),
        ..Default::default()
    };
    modal.fill_from(&listing_co());
    assert_eq!(modal.name, "ABC SDN. BHD.");
}
