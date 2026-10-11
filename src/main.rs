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
        [command, kind, width, height, depth, output] if command == "mesh-new" => (|| {
            let kind = effectcraft_model::prim::PrimitiveKind::parse(&kind.to_string_lossy())
                .ok_or_else(|| "unknown mesh primitive".to_string())?;
            let dimensions = [parse(width)?, parse(height)?, parse(depth)?];
            let document = modelstrike::editor::Document::new_primitive(kind, dimensions)?;
            document.save_new(Path::new(output))?;
            Ok(format!("editable mesh saved: {}", Path::new(output).display()))
        })(),
        [command, input] if command == "mesh-info" => (|| {
            let document = modelstrike::editor::Document::load(Path::new(input))?;
            let (bounds_min, bounds_max) = document.bounds().ok_or("mesh has no vertices")?;
            serde_json::to_string(&serde_json::json!({
                "objects": document.objects.len(),
                "triangles": document.triangle_count(),
                "materials": document.materials.len(),
                "bounds_min": bounds_min,
                "bounds_max": bounds_max,
                "units": document.units,
            })).map_err(|error| error.to_string())
        })(),
        [command, input, output, scale] if command == "mesh-import" => (|| {
            let (document, warnings) = modelstrike::editor::Document::import_model(Path::new(input), parse(scale)?)?;
            document.save_new(Path::new(output))?;
            for warning in warnings { eprintln!("import warning: {warning}"); }
            Ok(format!("imported editable mesh: {}", Path::new(output).display()))
        })(),
        [command, input, output, id, x, y, z] if command == "mesh-move-vertex" => (|| {
            let mut document = modelstrike::editor::Document::load(Path::new(input))?;
            let id = id.to_string_lossy().parse::<u64>().map_err(|_| "invalid vertex ID".to_string())?;
            document.move_vertex(id, [parse(x)?, parse(y)?, parse(z)?])?;
            document.save_new(Path::new(output))?;
            Ok(format!("edited mesh saved: {}", Path::new(output).display()))
        })(),
        [command, input, output, id, red, green, blue] if command == "mesh-set-color" => (|| {
            let mut document = modelstrike::editor::Document::load(Path::new(input))?;
            let id = id.to_string_lossy().parse::<u64>().map_err(|_| "invalid object ID".to_string())?;
            document.set_object_color(id, [parse(red)? as f32, parse(green)? as f32, parse(blue)? as f32, 1.0])?;
            document.save_new(Path::new(output))?;
            Ok(format!("colored mesh saved: {}", Path::new(output).display()))
        })(),
        [command, input, output, id, inset, raise] if command == "mesh-inset-raise-face" => (|| {
            let mut document = modelstrike::editor::Document::load(Path::new(input))?;
            let id = id.to_string_lossy().parse::<u64>().map_err(|_| "invalid triangle face ID".to_string())?;
            document.inset_raise_triangle(id, parse(inset)?, parse(raise)?)?;
            document.save_new(Path::new(output))?;
            Ok(format!("face-edited mesh saved: {}", Path::new(output).display()))
        })(),
        [command, input, output] if command == "mesh-export-obj" => (|| {
            let document = modelstrike::editor::Document::load(Path::new(input))?;
            document.export_obj_new(Path::new(output))?;
            Ok(format!("mesh OBJ saved: {}", Path::new(output).display()))
        })(),
        [command, input, output] if command == "mesh-export-glb" => (|| {
            let document = modelstrike::editor::Document::load(Path::new(input))?;
            let warnings = document.export_glb_new(Path::new(output))?;
            for warning in warnings { eprintln!("export warning: {warning}"); }
            Ok(format!("mesh GLB saved: {}", Path::new(output).display()))
        })(),
        _ => Err("usage: modelstrike inspect <prop.obj> <new-report.json> | new-cube <edge> <new.obj> | transform <input.obj> <new.obj> <dx> <dy> <dz> <scale> | mesh-new <primitive> <width_m> <height_m> <depth_m> <new.mstrike> | mesh-info <in.mstrike> | mesh-import <source.obj|gltf|glb> <new.mstrike> <obj-metres-per-unit> | mesh-move-vertex <in.mstrike> <new.mstrike> <id> <x> <y> <z> | mesh-set-color <in.mstrike> <new.mstrike> <object-id> <r> <g> <b> | mesh-inset-raise-face <in.mstrike> <new.mstrike> <face-id> <inset-fraction> <raise-m> | mesh-export-obj <in.mstrike> <new.obj> | mesh-export-glb <in.mstrike> <new.glb>".into()),
    };
    match result {
        Ok(message) => println!("{message}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
