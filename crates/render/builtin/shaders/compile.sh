compile() {
    local name=$1
    local stage=$2
    local entry=$3
    echo "Compiling ${name}.${stage}..."
    dxc -Fo "${name}.${stage}.dxil" "${name}.hlsl" -T "${stage}_6_7" -E "${entry}"
}

compile_raster() {
    local name=$1
    compile "${name}" ps mainPS
    compile "${name}" vs mainVS
}

compile_raster apply_hdri_as_background
compile_raster apply_hdri_as_light
compile_raster generate_shadow_mask
compile_raster immediate
compile_raster ssr
compile_raster cortao_apply
compile_raster overdraw

compile cortao_prepass cs main
compile cortao_main cs main
compile cortao_filter cs main
