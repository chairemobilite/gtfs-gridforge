use std::collections::HashMap;
use chrono::{NaiveTime, Datelike, Timelike};

const MAX_GRID_SIZE: usize = 26; // A-Z
const DEFAULT_SPACING_KM: f64 = 1.0;
const DEFAULT_BASE_LAT: f64 = 40.0;
const DEFAULT_BASE_LON: f64 = -74.0;

#[derive(Debug, Clone)]
pub struct GridPosition {
    row: char,    // Uppercase A-Z
    column: char, // Lowercase a-z
}

impl GridPosition {
    pub fn new(pos: &str) -> Option<Self> {
        let mut chars = pos.chars();
        let row = chars.next()?;
        let column = chars.next()?;
        
        if row.is_uppercase() && column.is_lowercase() &&
           row >= 'A' && row <= 'Z' && 
           column >= 'a' && column <= 'z' {
            Some(GridPosition { row, column })
        } else {
            None
        }
    }

    pub fn to_coordinates(&self, spacing_km: f64, base_lat: f64, base_lon: f64) -> (f64, f64) {
        let row_idx = (self.row as u8 - b'A') as f64;
        let col_idx = (self.column as u8 - b'a') as f64;
        
        // Convert grid positions to kilometers
        let x_km = col_idx * spacing_km;
        let y_km = row_idx * spacing_km;
        
        // Convert to approximate lat/lon
        // At the given latitude, adjust longitude conversion
        let lat_per_km = 1.0 / 111.32; // degrees per km
        let lon_per_km = 1.0 / (111.32 * base_lat.to_radians().cos()); // adjust for latitude
        
        let lat_delta = y_km * lat_per_km;
        let lon_delta = x_km * lon_per_km;
        
        (base_lat + lat_delta, base_lon + lon_delta)
    }

    pub fn to_string(&self) -> String {
        format!("{}{}", self.row, self.column)
    }
}

#[derive(Debug)]
pub struct GridConfig {
    pub spacing_km: f64,
    pub base_lat: f64,
    pub base_lon: f64,
}

impl Default for GridConfig {
    fn default() -> Self {
        GridConfig {
            spacing_km: DEFAULT_SPACING_KM,
            base_lat: DEFAULT_BASE_LAT,
            base_lon: DEFAULT_BASE_LON,
        }
    }
}

#[derive(Debug)]
pub struct GridStop {
    pub id: String,
    pub name: String,
    pub position: GridPosition,
}

#[derive(Debug)]
pub struct GridRoute {
    pub id: String,
    pub name: String,
    pub stops: Vec<String>, // Stop IDs in sequence
    pub schedule: Vec<NaiveTime>, // List of first departure times
    pub frequency: i32,    // Frequency in minutes
}

#[derive(Debug)]
pub struct TransitGrid {
    pub stops: HashMap<String, GridStop>,
    pub routes: Vec<GridRoute>,
    pub config: GridConfig,
}

impl TransitGrid {
    pub fn new(config: Option<GridConfig>) -> Self {
        TransitGrid {
            stops: HashMap::new(),
            routes: Vec::new(),
            config: config.unwrap_or_default(),
        }
    }

    pub fn add_stop(&mut self, position: &str, name: String) -> Result<(), String> {
        let grid_pos = GridPosition::new(position)
            .ok_or_else(|| format!("Invalid grid position: {}", position))?;
            
        let stop_id = grid_pos.to_string();
        let stop = GridStop {
            id: stop_id.clone(),
            name,
            position: grid_pos,
        };
        self.stops.insert(stop_id, stop);
        Ok(())
    }

    pub fn add_route(&mut self, id: String, name: String, stops: Vec<String>, 
                 first_departure: NaiveTime, frequency: i32,
                 generate_reverse: bool) -> Result<(), String> {
        // Validate all stops exist
        for stop_id in &stops {
            if !self.stops.contains_key(stop_id) {
                return Err(format!("Stop {} not found", stop_id));
            }
        }
        
        // Forward direction
        let forward_route = GridRoute {
            id: id.clone(),
            name: name.clone(),
            stops: stops.clone(),
            schedule: vec![first_departure],
            frequency,
            // We'll use this field in the GTFS generation to set direction_id=0
            // No need to store it explicitly here as we can infer it from _reverse suffix
        };
        self.routes.push(forward_route);
        
        // Generate reverse direction if requested
        if generate_reverse {
            // Reverse the stops array
            let mut reverse_stops = stops;
            reverse_stops.reverse();
            
            // Add offset to departure time (typically half the frequency)
            let reverse_departure = first_departure + chrono::Duration::minutes((frequency / 2) as i64);
            
            // Create reverse route with same ID but marked internally for direction=1
            let reverse_route = GridRoute {
                id: format!("{}_reverse", id), // We'll use this suffix to identify reverse routes
                name: name, // Same name for both directions as per GTFS best practices
                stops: reverse_stops,
                schedule: vec![reverse_departure],
                frequency,
            };
            self.routes.push(reverse_route);
        }
        
        Ok(())
    }

    pub fn generate_gtfs(&self, output_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Create the directory if it doesn't exist
        std::fs::create_dir_all(output_dir)?;
        
        // Instead of using RawGtfs, we'll directly create the CSV files
        let mut stops = Vec::new();
        let mut routes = Vec::new();
        let mut trips = Vec::new();
        let mut stop_times = Vec::new();
        let mut calendar = Vec::new();
        
        // Add headers
        stops.push("stop_id,stop_name,stop_lat,stop_lon".to_string());
        routes.push("route_id,route_short_name,route_long_name,route_type".to_string());
        trips.push("route_id,service_id,trip_id,direction_id".to_string());
        stop_times.push("trip_id,arrival_time,departure_time,stop_id,stop_sequence".to_string());
        
        // Create calendar header
        let current_year = chrono::Local::now().year();
        let start_date = format!("{:04}0101", current_year); // YYYYMMDD
        let end_date = format!("{:04}1231", current_year);
        
        calendar.push("service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date".to_string());
        calendar.push(format!("WEEKDAY,1,1,1,1,1,0,0,{},{}",
            start_date, end_date));
        
        // Create stops
        for stop in self.stops.values() {
            let (lat, lon) = stop.position.to_coordinates(
                self.config.spacing_km,
                self.config.base_lat,
                self.config.base_lon
            );
            
            stops.push(format!("{},{},{},{}",
                stop.id, stop.name, lat, lon));
        }
        
        // Create routes
        for route in &self.routes {
            // Extract the parent route ID (removing _reverse suffix if present)
            let parent_route_id = if route.id.ends_with("_reverse") {
                route.id.strip_suffix("_reverse").unwrap_or(&route.id)
            } else {
                &route.id
            };
            
            // Only add each unique route once
            if !routes.iter().any(|r| r.contains(&format!("{},", parent_route_id))) {
                routes.push(format!("{},{},{},{}",
                    parent_route_id, route.name, "", 3)); // 3 = bus
            }
        }
        
        // Create trips and stop times
        for route in &self.routes {
            for &start_time in route.schedule.iter() {
                // Generate trips based on frequency
                // Assuming service from start_time until midnight
                let end_time = NaiveTime::from_hms_opt(23, 59, 0).unwrap();
                let mut departure_time = start_time;
                let mut trip_index = 0;
                
                // Create trips at the specified frequency
                // Limit to a reasonable number of trips (e.g., 300) to prevent infinite loops
                let max_trips = 300;
                
                while departure_time < end_time && trip_index < max_trips {
                    // Determine direction_id (0 for forward, 1 for reverse)
                    let direction_id = if route.id.ends_with("_reverse") { 1 } else { 0 };
                    
                    // Get the parent route ID (without _reverse suffix)
                    let parent_route_id = if route.id.ends_with("_reverse") {
                        route.id.strip_suffix("_reverse").unwrap_or(&route.id)
                    } else {
                        &route.id
                    };
                    
                    let trip_id = format!("{}_{}_dir{}", parent_route_id, trip_index, direction_id);
                    
                    trips.push(format!("{},WEEKDAY,{},{}",
                        parent_route_id, trip_id, direction_id));
                    
                    let mut current_time = departure_time;
                    for (stop_sequence, stop_id) in route.stops.iter().enumerate() {
                        // Format time as HH:MM:SS
                        let time_str = format!(
                            "{:02}:{:02}:{:02}", 
                            current_time.hour(), 
                            current_time.minute(), 
                            current_time.second()
                        );
                        
                        stop_times.push(format!(
                            "{},{},{},{},{}",
                            trip_id, time_str, time_str, stop_id, stop_sequence
                        ));
                        
                        // Calculate time to next stop based on distance if there is a next stop
                        if stop_sequence + 1 < route.stops.len() {
                            let current_stop = &self.stops[stop_id];
                            let next_stop = &self.stops[&route.stops[stop_sequence + 1]];
                            
                            // Calculate minutes between stops based on distance
                            // Assuming average speed of 30 km/h
                            let (lat1, lon1) = current_stop.position.to_coordinates(
                                self.config.spacing_km,
                                self.config.base_lat,
                                self.config.base_lon
                            );
                            let (lat2, lon2) = next_stop.position.to_coordinates(
                                self.config.spacing_km,
                                self.config.base_lat,
                                self.config.base_lon
                            );
                            
                            // Simplified distance calculation
                            let dlat = lat2 - lat1;
                            let dlon = lon2 - lon1;
                            let dist_km = ((dlat * dlat + dlon * dlon).sqrt() * 111.32).min(10.0);
                            
                            // At 30 km/h, calculate minutes
                            let minutes = ((dist_km / 30.0) * 60.0).ceil() as i64;
                            current_time = current_time + chrono::Duration::minutes(minutes);
                        }
                    }
                    
                    // Increment trip index and departure time for the next trip
                    trip_index += 1;
                    departure_time = departure_time + chrono::Duration::minutes(route.frequency as i64);
                }
            }
        }
        
        // Write files
        std::fs::write(format!("{}/stops.txt", output_dir), stops.join("\n"))?;
        std::fs::write(format!("{}/routes.txt", output_dir), routes.join("\n"))?;
        std::fs::write(format!("{}/trips.txt", output_dir), trips.join("\n"))?;
        std::fs::write(format!("{}/stop_times.txt", output_dir), stop_times.join("\n"))?;
        std::fs::write(format!("{}/calendar.txt", output_dir), calendar.join("\n"))?;
        
        // Create agency.txt (required for valid GTFS)
        let agency_content = "agency_id,agency_name,agency_url,agency_timezone\nAGENCY,GridForge Transit,http://example.com,America/New_York";
        std::fs::write(format!("{}/agency.txt", output_dir), agency_content)?;
        
        Ok(())
    }
}
