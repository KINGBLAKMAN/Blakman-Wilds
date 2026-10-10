"""Emberwild asset kit: shared helpers for scripted low-poly assets (Blender 5.x).

Conventions (match the game):
- 1 Blender unit = 1 metre. Blender is Z-up; the glTF exporter converts to Y-up for Bevy.
- Each asset is built in its own collection named after the asset, origin at the base centre
  (feet / trunk base) so the game places it on the ground directly.
- Flat-shaded low poly, colours from one shared palette (one material per palette colour).
- Export one .glb per asset with `export_asset(name, path)`.

Use inside Blender:  exec(open(r"<this file>").read())
"""
import math
import random

import bmesh
import bpy
from mathutils import Matrix, Vector

PALETTE = {
    "bark": (0.30, 0.19, 0.11),
    "bark_dark": (0.20, 0.12, 0.07),
    "leaf": (0.27, 0.52, 0.22),
    "leaf_dark": (0.17, 0.38, 0.17),
    "leaf_light": (0.45, 0.66, 0.27),
    "stone": (0.48, 0.48, 0.50),
    "stone_dark": (0.32, 0.32, 0.35),
    "iron": (0.62, 0.38, 0.24),
    "berry": (0.78, 0.12, 0.22),
    "grass": (0.35, 0.58, 0.24),
    "sand": (0.86, 0.78, 0.55),
    "water": (0.18, 0.45, 0.65),
    "skin": (0.87, 0.67, 0.53),
    "cloth": (0.20, 0.34, 0.58),
    "cloth_red": (0.62, 0.18, 0.15),
    "leather": (0.42, 0.26, 0.14),
    "hair": (0.24, 0.15, 0.09),
    "wolf": (0.40, 0.40, 0.43),
    "wolf_light": (0.70, 0.70, 0.72),
    "eye": (0.95, 0.80, 0.20),
    "ember": (1.00, 0.45, 0.08),
    "metal": (0.55, 0.57, 0.60),
    "wood": (0.55, 0.38, 0.22),
    "black": (0.05, 0.05, 0.05),
}


def mat(name):
    """One shared material per palette colour (flat base colour, slightly rough)."""
    key = f"EW_{name}"
    m = bpy.data.materials.get(key)
    if m is None:
        m = bpy.data.materials.new(key)
        m.use_nodes = True
        bsdf = m.node_tree.nodes.get("Principled BSDF")
        r, g, b = PALETTE[name]
        bsdf.inputs["Base Color"].default_value = (r, g, b, 1.0)
        bsdf.inputs["Roughness"].default_value = 0.85
        if name == "ember":
            bsdf.inputs["Emission Color"].default_value = (r, g, b, 1.0)
            bsdf.inputs["Emission Strength"].default_value = 4.0
        m.diffuse_color = (r, g, b, 1.0)
    return m


def clear_scene():
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o, do_unlink=True)
    for c in list(bpy.data.collections):
        bpy.data.collections.remove(c)
    for block in (bpy.data.meshes, bpy.data.armatures, bpy.data.actions):
        for item in list(block):
            if item.users == 0:
                block.remove(item)


def collection(name):
    c = bpy.data.collections.get(name) or bpy.data.collections.new(name)
    if c.name not in bpy.context.scene.collection.children:
        bpy.context.scene.collection.children.link(c)
    return c


def _finish(obj, coll, material, smooth=False):
    coll.objects.link(obj)
    if material:
        obj.data.materials.append(mat(material))
    for p in obj.data.polygons:
        p.use_smooth = smooth
    return obj


def mesh_obj(name, bm, coll, material, smooth=False):
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    return _finish(bpy.data.objects.new(name, me), coll, material, smooth)


def cone(name, coll, material, r1, r2, depth, segments=7, loc=(0, 0, 0), rot=(0, 0, 0)):
    """Cylinder/cone standing on its base (base at loc)."""
    bm = bmesh.new()
    bmesh.ops.create_cone(bm, cap_ends=True, segments=segments, radius1=r1, radius2=r2, depth=depth)
    bmesh.ops.translate(bm, verts=bm.verts, vec=(0, 0, depth / 2))
    o = mesh_obj(name, bm, coll, material)
    o.location, o.rotation_euler = loc, rot
    return o


def blob(name, coll, material, radius, loc=(0, 0, 0), scale=(1, 1, 1), subdiv=1, jitter=0.12, seed=0):
    """Low-poly lumpy sphere (icosphere with seeded vertex jitter): foliage, rocks, bushes."""
    rng = random.Random(seed)
    bm = bmesh.new()
    bmesh.ops.create_icosphere(bm, subdivisions=subdiv, radius=radius)
    for v in bm.verts:
        v.co += v.co.normalized() * radius * rng.uniform(-jitter, jitter)
        v.co = Vector((v.co.x * scale[0], v.co.y * scale[1], v.co.z * scale[2]))
    o = mesh_obj(name, bm, coll, material)
    o.location = loc
    return o


def box(name, coll, material, size, loc=(0, 0, 0), rot=(0, 0, 0), bevel=0.0):
    """Box with its base at loc (size = full x, y, z)."""
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    bmesh.ops.scale(bm, verts=bm.verts, vec=size)
    bmesh.ops.translate(bm, verts=bm.verts, vec=(0, 0, size[2] / 2))
    if bevel > 0:
        bmesh.ops.bevel(bm, geom=bm.edges[:], offset=bevel, segments=1, affect="EDGES")
    o = mesh_obj(name, bm, coll, material)
    o.location, o.rotation_euler = loc, rot
    return o


def join(objs, name):
    """Join parts into one mesh object named `name` (materials kept per face)."""
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    bpy.ops.object.join()
    o = bpy.context.view_layer.objects.active
    o.name = name
    o.data.name = name
    bpy.ops.object.transform_apply(location=False, rotation=True, scale=True)
    return o


def stats(coll_name):
    tris = 0
    for o in bpy.data.collections[coll_name].all_objects:
        if o.type == "MESH":
            tris += sum(len(p.vertices) - 2 for p in o.data.polygons)
    return tris


def export_asset(coll_name, path, animations=False):
    """Export one collection to .glb (Y-up conversion, applied modifiers, flat normals kept)."""
    bpy.ops.object.select_all(action="DESELECT")
    for o in bpy.data.collections[coll_name].all_objects:
        o.select_set(True)
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLB",
        use_selection=True,
        export_yup=True,
        export_apply=True,
        export_animations=animations,
        export_materials="EXPORT",
    )
    return path
