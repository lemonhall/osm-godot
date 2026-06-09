extends SceneTree

func _initialize() -> void:
	call_deferred("_run")

func _run() -> void:
	var loader_script: Script = load("res://scripts/chunk_mesh_loader.gd")
	if loader_script == null:
		_fail("missing_chunk_loader_script")
		return

	var mesh_path := "user://building_plaques_e2e_mesh.json"
	var file := FileAccess.open(mesh_path, FileAccess.WRITE)
	if file == null:
		_fail("failed_to_write_mesh_json")
		return
	file.store_string(JSON.stringify({
		"elements": [
			_building_element("BuildingWall_zh", "zh-1", {"name:zh": "建筑科学院", "building": "office"}, 0.0),
			_building_element("BuildingRoof_zh", "zh-1", {"name:zh": "建筑科学院", "building": "office"}, 0.0),
			_building_element("BuildingWall_en", "en-1", {"name": "Science Building", "building": "office"}, 8.0),
		]
	}))
	file.close()

	var root := Node3D.new()
	root.name = "PlaqueE2ERoot"
	get_root().add_child(root)
	current_scene = root

	var chunk := Node3D.new()
	chunk.name = "PlaqueE2EChunk"
	chunk.set_script(loader_script)
	chunk.set("mesh_data_path", mesh_path)
	root.add_child(chunk)

	for i in range(180):
		await process_frame
		if bool(chunk.get_meta("chunk_loading_complete", false)):
			break

	var plaques := []
	_collect_plaques(chunk, plaques)
	var plaque_count := plaques.size()
	var chinese_count := 0
	var english_count := 0
	var has_background := false
	var has_label := false
	var label_text := ""
	for plaque in plaques:
		var text := str(plaque.get_meta("plaque_text", ""))
		if text == "建筑科学院":
			chinese_count += 1
			has_background = plaque.get_node_or_null("PlaqueBackground") is MeshInstance3D
			var label := plaque.get_node_or_null("PlaqueText") as Label3D
			has_label = label != null
			if label != null:
				label_text = label.text
		if text == "Science Building":
			english_count += 1

	print("PLAQUE_E2E plaque_count=", plaque_count)
	print("PLAQUE_E2E chinese_count=", chinese_count)
	print("PLAQUE_E2E english_count=", english_count)
	print("PLAQUE_E2E has_background=", has_background)
	print("PLAQUE_E2E has_label=", has_label)
	print("PLAQUE_E2E label_text=", label_text.replace("\n", "|"))

	var failed := false
	if plaque_count != 1:
		push_error("PLAQUE_E2E expected exactly one deduplicated plaque")
		failed = true
	if chinese_count != 1:
		push_error("PLAQUE_E2E missing Chinese plaque")
		failed = true
	if english_count != 0:
		push_error("PLAQUE_E2E English-only building should not receive plaque")
		failed = true
	if not has_background or not has_label:
		push_error("PLAQUE_E2E plaque missing background or Label3D")
		failed = true

	quit(1 if failed else 0)

func _building_element(name: String, osm_id: String, metadata_extra: Dictionary, x_offset: float) -> Dictionary:
	var metadata := {
		"osm_id": osm_id,
		"osm_kind": "building",
	}
	for key in metadata_extra.keys():
		metadata[key] = metadata_extra[key]
	return {
		"name": name,
		"material": "building_wall",
		"transform": [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, x_offset, 0.0, 0.0],
		"metadata": metadata,
		"vertices": [0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0, 3.0, 0.0, 0.0, 3.0, 0.0],
		"normals": [0.0, 0.0, -1.0, 0.0, 0.0, -1.0, 0.0, 0.0, -1.0, 0.0, 0.0, -1.0],
		"uvs": [0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0],
		"indices": [0, 1, 2, 0, 2, 3],
	}

func _collect_plaques(node: Node, out: Array) -> void:
	if node.get_meta("building_plaque", false):
		out.append(node)
	for child in node.get_children():
		_collect_plaques(child, out)

func _fail(reason: String) -> void:
	push_error("PLAQUE_E2E " + reason)
	quit(1)
