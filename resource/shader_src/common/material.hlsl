#ifndef _MATERIAL_HLSL_
#define _MATERIAL_HLSL_

// Same in the raven-render::renderer::mesh_renderer::add_asset_mesh::UploadMaterial (it is a anonymous struct inside a function)
struct Material {
    float metallic;
    float roughness;
    float base_color[4];
    float emissive[3];
    uint  albedo_map;
    uint  normal_map;
    uint  specular_map;
    uint  emissive_map;
    float texture_transform[6 * 4];
};

#endif