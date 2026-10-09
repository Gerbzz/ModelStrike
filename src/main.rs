use std::path::Path;

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let parse = |value: &std::ffi::OsString| {
        value
            .to_string_lossy()
            .parse::<f64>()
            .map_err(|_| "transform values must be numbers".to_string())
    };
    let result: Result<String, String> = match args.as_slice() {
        [command, input, output] if command == "inspect" => {
            modelstrike::inspect_obj(Path::new(input)).and_then(|report| {
                modelstrike::write_new_report(&report, Path::new(output))?;
                Ok(format!("{} vertices, {} faces, {} triangles; {} warnings",
                    report.vertices, report.faces, report.triangles_after_fan, report.warnings.len()))
            })
        }
        [command, edge, output] if command == "new-cube" => (|| {
            modelstrike::mesh::create_cube(parse(edge)?, Path::new(output))?;
            Ok(format!("new cube OBJ: {}", Path::new(output).display()))
        })(),
        [command, input, output, dx, dy, dz, scale] if command == "transform" => (|| {
            modelstrike::mesh::transform_obj(Path::new(input), Path::new(output),
                [parse(dx)?, parse(dy)?, parse(dz)?], parse(scale)?)?;
            Ok(format!("transformed OBJ: {}", Path::new(output).display()))
        })(),
        _ => Err("usage: modelstrike inspect <prop.obj> <new-report.json> | new-cube <edge> <new.obj> | transform <input.obj> <new.obj> <dx> <dy> <dz> <scale>".into()),
    };
    match result {
        Ok(message) => println!("{message}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
