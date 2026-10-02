use meridian_shared_types::{Point, RouteProgress, RouteSpec, TypedError};
const R: f64 = 6_371_008.8;
pub fn distance(a: &Point, b: &Point) -> f64 {
    let dlat = (b.latitude - a.latitude).to_radians();
    let dlon = (b.longitude - a.longitude).to_radians();
    let h = (dlat / 2.).sin().powi(2)
        + a.latitude.to_radians().cos() * b.latitude.to_radians().cos() * (dlon / 2.).sin().powi(2);
    2. * R * h.clamp(0., 1.).sqrt().asin()
}
pub fn bearing(a: &Point, b: &Point) -> f64 {
    let d = (b.longitude - a.longitude).to_radians();
    let a = a.latitude.to_radians();
    let b = b.latitude.to_radians();
    (d.sin() * b.cos())
        .atan2(a.cos() * b.sin() - a.sin() * b.cos() * d.cos())
        .to_degrees()
        .rem_euclid(360.)
}
pub fn step(a: &Point, bearing_degrees: f64, meters: f64) -> Result<Point, TypedError> {
    a.validate()?;
    if !bearing_degrees.is_finite() || !meters.is_finite() || !(0.0..=10_000.0).contains(&meters) {
        return Err(TypedError::invalid(
            "Joystick distance must be 0–10,000 metres and bearing must be finite.",
        ));
    }
    Ok(destination(a, bearing_degrees, meters))
}
fn destination(a: &Point, bearing_degrees: f64, meters: f64) -> Point {
    let lat = a.latitude.to_radians();
    let lon = a.longitude.to_radians();
    let b = bearing_degrees.to_radians();
    let d = meters / R;
    let lat2 = (lat.sin() * d.cos() + lat.cos() * d.sin() * b.cos())
        .clamp(-1., 1.)
        .asin();
    let lon2 = lon + (b.sin() * d.sin() * lat.cos()).atan2(d.cos() - lat.sin() * lat2.sin());
    Point {
        latitude: lat2.to_degrees(),
        longitude: (lon2.to_degrees() + 180.).rem_euclid(360.) - 180.,
    }
}
pub fn validate_speed(speed: f64) -> Result<(), TypedError> {
    if !speed.is_finite() || !(0.1..=100.).contains(&speed) {
        Err(TypedError::invalid(
            "Route speed must be 0.1–100 metres per second.",
        ))
    } else {
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct Route {
    pub spec: RouteSpec,
    cumulative: Vec<f64>,
    pub acknowledged_distance: f64,
    pub paused: bool,
    pub finished: bool,
}
impl Route {
    pub fn new(spec: RouteSpec) -> Result<Self, TypedError> {
        if spec.id.is_empty()
            || spec.id.len() > 128
            || spec.name.len() > 256
            || !(2..=100_000).contains(&spec.points.len())
        {
            return Err(TypedError::invalid(
                "A route needs an identifier and 2–100,000 points.",
            ));
        }
        validate_speed(spec.speed_mps)?;
        for p in &spec.points {
            p.validate()?;
        }
        let mut cumulative = vec![0.];
        for w in spec.points.windows(2) {
            cumulative.push(cumulative.last().unwrap() + distance(&w[0], &w[1]));
        }
        if *cumulative.last().unwrap() < 0.01 {
            return Err(TypedError::invalid(
                "Route points must span a nonzero distance.",
            ));
        }
        if spec.r#loop && distance(spec.points.first().unwrap(), spec.points.last().unwrap()) > 1. {
            return Err(TypedError::invalid(
                "A looping route must end within one metre of its start.",
            ));
        }
        Ok(Self {
            spec,
            cumulative,
            acknowledged_distance: 0.,
            paused: false,
            finished: false,
        })
    }
    pub fn total(&self) -> f64 {
        *self.cumulative.last().unwrap()
    }
    pub fn point_at(&self, meters: f64) -> Point {
        let d = meters.clamp(0., self.total());
        let i = self
            .cumulative
            .partition_point(|x| *x <= d)
            .saturating_sub(1)
            .min(self.spec.points.len() - 2);
        if d >= self.total() {
            return self.spec.points.last().unwrap().clone();
        }
        destination(
            &self.spec.points[i],
            bearing(&self.spec.points[i], &self.spec.points[i + 1]),
            d - self.cumulative[i],
        )
    }
    pub fn proposal(&self, elapsed_seconds: f64) -> (f64, Point) {
        let mut d = self.acknowledged_distance;
        if !self.paused && !self.finished {
            d += elapsed_seconds.clamp(0., 1.) * self.spec.speed_mps;
            if self.spec.r#loop {
                d = d.rem_euclid(self.total());
            } else {
                d = d.min(self.total());
            }
        }
        (d, self.point_at(d))
    }
    pub fn acknowledge(&mut self, distance: f64) {
        self.acknowledged_distance = distance;
        self.finished = !self.spec.r#loop && distance >= self.total();
    }
    pub fn progress(&self) -> RouteProgress {
        RouteProgress {
            id: self.spec.id.clone(),
            name: self.spec.name.clone(),
            distance_meters: self.acknowledged_distance,
            total_meters: self.total(),
            speed_mps: self.spec.speed_mps,
            paused: self.paused,
            finished: self.finished,
            r#loop: self.spec.r#loop,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn route() -> Route {
        Route::new(RouteSpec {
            id: "r".into(),
            name: "r".into(),
            points: vec![
                Point {
                    latitude: 0.,
                    longitude: 179.999,
                },
                Point {
                    latitude: 0.,
                    longitude: -179.999,
                },
            ],
            speed_mps: 10.,
            r#loop: false,
        })
        .unwrap()
    }
    #[test]
    fn short_antimeridian_path() {
        let r = route();
        assert!((r.total() - 222.39).abs() < 0.1);
        assert!(r.point_at(r.total() / 2.).longitude.abs() > 179.99);
    }
    #[test]
    fn proposals_do_not_advance_acknowledged_checkpoint() {
        let r = route();
        let (d, _) = r.proposal(0.5);
        assert_eq!(d, 5.);
        assert_eq!(r.acknowledged_distance, 0.);
    }
    #[test]
    fn pause_resume_and_speed_preserve_checkpoint() {
        let mut r = route();
        r.acknowledge(5.);
        r.paused = true;
        assert_eq!(r.proposal(1.).0, 5.);
        r.paused = false;
        r.spec.speed_mps = 20.;
        assert_eq!(r.proposal(0.5).0, 15.);
    }
    #[test]
    fn long_suspend_is_bounded() {
        assert_eq!(route().proposal(1000.).0, 10.);
    }
    #[test]
    fn bearings_and_joystick_are_geodesic() {
        let a = Point {
            latitude: 0.,
            longitude: 0.,
        };
        let b = step(&a, 90., 100.).unwrap();
        assert!((distance(&a, &b) - 100.).abs() < 0.001);
        assert!((bearing(&a, &b) - 90.).abs() < 0.001);
    }
    #[test]
    fn rejects_jump_loop() {
        let mut r = route().spec;
        r.r#loop = true;
        assert!(Route::new(r).is_err());
    }
}
