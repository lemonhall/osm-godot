extends SceneTree

func _initialize() -> void:
	call_deferred("_run")

func _run() -> void:
	var packed: PackedScene = load("res://scenes/master.tscn")
	if packed == null:
		_fail("failed_to_load_master")
		return

	var scene: Node = packed.instantiate()
	get_root().add_child(scene)
	current_scene = scene

	for i in range(24):
		await process_frame
		await physics_frame

	var player: CharacterBody3D = scene.get_node_or_null("Player")
	if player == null:
		_fail("missing_player")
		return
	var controller: Node = scene.get_node_or_null("NavigationController")
	if controller == null:
		_fail("missing_navigation_controller")
		return

	var ok: bool = controller.call("start_navigation_to_query", "外滩")
	if not ok:
		ok = controller.call("start_navigation_to_query", "外滩源")
	if not ok:
		_fail("route_start_failed status=" + str(controller.call("get_navigation_status")))
		return

	for i in range(12):
		await process_frame
		await physics_frame

	var graph_nodes := int(controller.call("get_graph_node_count"))
	var graph_edges := int(controller.call("get_graph_edge_count"))
	var waypoint_count := int(controller.call("get_route_waypoint_count"))
	var total_distance := float(controller.call("get_route_total_distance"))
	var instruction := str(controller.call("get_current_instruction"))
	var hud := controller.get_node_or_null("NavigationHUD")
	var overlay := controller.get_node_or_null("RouteOverlay")
	var line := controller.get_node_or_null("RouteOverlay/RouteLine")
	var arrow := controller.get_node_or_null("RouteOverlay/RouteArrow")

	print("NAV_E2E graph_nodes=", graph_nodes)
	print("NAV_E2E graph_edges=", graph_edges)
	print("NAV_E2E waypoint_count=", waypoint_count)
	print("NAV_E2E total_distance=", total_distance)
	print("NAV_E2E instruction=", instruction)
	print("NAV_E2E status=", controller.call("get_navigation_status"))
	print("NAV_E2E hud_exists=", hud != null)
	print("NAV_E2E overlay_exists=", overlay != null)
	print("NAV_E2E route_line_exists=", line != null)
	print("NAV_E2E route_arrow_exists=", arrow != null)

	var failed := false
	if graph_nodes <= 0:
		push_error("NAV_E2E graph has no nodes")
		failed = true
	if graph_edges <= 0:
		push_error("NAV_E2E graph has no edges")
		failed = true
	if waypoint_count <= 2:
		push_error("NAV_E2E route should contain more than a direct two-point line")
		failed = true
	if total_distance <= 0.0:
		push_error("NAV_E2E route distance is zero")
		failed = true
	if instruction.is_empty():
		push_error("NAV_E2E instruction is empty")
		failed = true
	if hud == null or overlay == null or line == null or arrow == null:
		push_error("NAV_E2E missing navigation HUD or overlay nodes")
		failed = true

	quit(1 if failed else 0)

func _fail(reason: String) -> void:
	push_error("NAV_E2E " + reason)
	quit(1)
