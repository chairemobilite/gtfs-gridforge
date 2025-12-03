use gtfs_gridforge::{TransitGrid, GridConfig};
use chrono::NaiveTime;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a grid with custom configuration for Montreal
    let config = GridConfig {
        spacing_km: 1.5,                // 1.5km spacing between grid points
        base_lat: 45.5017,              // Montreal downtown coordinates
        base_lon: -73.5673,
    };
    let mut grid = TransitGrid::new(Some(config));

    // Add some stops using the letter-based grid system
    // Real Montreal locations
    grid.add_stop("Aa", "Place-des-Arts".into())?;
    grid.add_stop("Bd", "McGill".into())?;
    grid.add_stop("Ce", "Peel".into())?;
    grid.add_stop("Dg", "Guy-Concordia".into())?;
    grid.add_stop("Ei", "Atwater".into())?;

    // Add routes based on Montreal metro Green Line
    let first_departure = NaiveTime::from_hms_opt(5, 30, 0).unwrap();
    
    let green_stops = vec!["Aa".into(), "Bd".into(), "Ce".into(), "Dg".into(), "Ei".into()];
    grid.add_route("GREEN".into(), "Green Line".into(), green_stops, first_departure, 5, true)?;

    // Add a second route (Orange Line segment)
    grid.add_stop("Af", "Berri-UQAM".into())?;
    grid.add_stop("Ff", "Sherbrooke".into())?;
    grid.add_stop("Fm", "Mont-Royal".into())?;
    
    let orange_departure = NaiveTime::from_hms_opt(5, 15, 0).unwrap();
    let orange_stops = vec!["Af".into(), "Ff".into(), "Fm".into()];
    grid.add_route("ORANGE".into(), "Orange Line".into(), orange_stops, orange_departure, 4, true)?;

    // Generate GTFS files
    grid.generate_gtfs("output")?;

    println!("GTFS files generated successfully in 'output' directory");
    println!("Created a simulated Montreal metro system with Green and Orange lines");

    Ok(())
}
