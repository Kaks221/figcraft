//! Commands never leave a layer with a non-finite position or size: such a file saves but can't be
//! reopened (JSON writes NaN and infinity as null). Only the layers a command changed are checked
//! (#1672): the check still finds them among many, and a layer already broken in an old file that
//! the command leaves alone doesn't block it.

use serde_json::json;

use crate::Session;

fn rect(s: &mut Session) -> crate::NodeId {
    let r = s.execute("node.create", &json!({"type": "rectangle", "x": 0, "y": 10, "width": 100, "height": 50})).unwrap();
    crate::params::id_of(&r["id"]).unwrap()
}

fn assert_roundtrips(s: &Session) {
    let bytes = figcraft_format::save_bytes(s.doc().unwrap(), None).unwrap();
    assert!(figcraft_format::load_bytes(&bytes).is_ok(), "the saved file reopens");
}

#[test]
fn huge_moves_are_refused_and_the_file_still_reopens() {
    let mut s = Session::new();
    let r = rect(&mut s);
    s.execute("transform.move", &json!({"ids": [r], "dx": 1e308, "dy": 0})).ok();
    let second = s.execute("transform.move", &json!({"ids": [r], "dx": 1e308, "dy": 1e308}));
    assert!(second.is_err(), "a move past the f64 range is refused");
    let n = s.doc().unwrap().node(r).unwrap();
    assert!(n.transform.as_coeffs().iter().all(|c| c.is_finite()), "{:?}", n.transform);
    assert_roundtrips(&s);
}

#[test]
fn huge_scale_is_refused_and_the_file_still_reopens() {
    let mut s = Session::new();
    let r = rect(&mut s);
    s.execute("transform.scale", &json!({"ids": [r], "factor": 1e300})).ok();
    let second = s.execute("transform.scale", &json!({"ids": [r], "factor": 1e300}));
    assert!(second.is_err(), "a size past the f64 range is refused");
    let n = s.doc().unwrap().node(r).unwrap();
    assert!(n.width.is_finite() && n.height.is_finite(), "{} x {}", n.width, n.height);
    assert_roundtrips(&s);
}

/// A frame holding `n` rectangles, and the last of them.
fn crowd(s: &mut Session, n: usize) -> crate::NodeId {
    let f = s.execute("node.create", &json!({"type": "frame", "x": 0, "y": 0, "width": 4000, "height": 4000})).unwrap();
    let f = crate::params::id_of(&f["id"]).unwrap();
    let mut last = f;
    for i in 0..n {
        let r = s
            .execute("node.create", &json!({"type": "rectangle", "parent": f, "x": (i % 60) * 60, "y": (i / 60) * 60, "width": 40, "height": 40}))
            .unwrap();
        last = crate::params::id_of(&r["id"]).unwrap();
    }
    last
}

#[test]
fn a_changed_layer_among_many_is_still_checked() {
    let mut s = Session::new();
    let deep = crowd(&mut s, 2000);
    s.execute("transform.move", &json!({"ids": [deep], "dx": 1e308, "dy": 0})).ok();
    assert!(s.execute("transform.move", &json!({"ids": [deep], "dx": 1e308, "dy": 1e308})).is_err());
    let n = s.doc().unwrap().node(deep).unwrap();
    assert!(n.transform.as_coeffs().iter().all(|c| c.is_finite()), "{:?}", n.transform);
    assert_roundtrips(&s);
}

#[test]
fn a_layer_already_broken_blocks_only_edits_of_itself() {
    let mut s = Session::new();
    let r = rect(&mut s);
    let other = rect(&mut s);
    // As an old file could hold it: a NaN width the check never let a command write.
    s.doc_mut().unwrap().get_mut(r).unwrap().width = f64::NAN;
    s.execute("node.set", &json!({"ids": [other], "props": {"name": "Fine"}})).unwrap();
    assert_eq!(s.doc().unwrap().node(other).unwrap().name, "Fine", "an edit elsewhere goes through");
    let e = s.execute("node.set", &json!({"ids": [r], "props": {"name": "Still broken"}})).unwrap_err();
    assert!(e.to_string().contains("beyond the number range"), "{e}");
}
