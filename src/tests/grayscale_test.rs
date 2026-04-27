#[cfg(test)]
mod tests {
    use crate::plugins::filters::grayscale::{convert_to_gray, grayscale_system, GrayscaleTag};
    use ndarray::Array3;

    #[test]
    fn white_pixel() {
        let input = Array3::from_elem((1, 1, 3), 255u8);
        let out = convert_to_gray(&input);
        assert_eq!(out.shape(), &[1, 1, 1]);
        assert_eq!(out[[0, 0, 0]], 255);
    }

    #[test]
    fn black_pixel() {
        let input = Array3::zeros((1, 1, 3));
        let out = convert_to_gray(&input);
        assert_eq!(out[[0, 0, 0]], 0);
    }

    #[test]
    fn pure_red() {
        // R=255, G=0, B=0 → Y = 0.299 * 255 ≈ 76
        let mut input = Array3::zeros((1, 1, 3));
        input[[0, 0, 0]] = 255;
        let out = convert_to_gray(&input);
        assert_eq!(out[[0, 0, 0]], (0.299f32 * 255.0) as u8); // 76
    }

    #[test]
    fn output_shape() {
        let input = Array3::from_elem((4, 6, 3), 128u8);
        let out = convert_to_gray(&input);
        assert_eq!(out.shape(), &[4, 6, 1]);
    }

    #[test]
    fn grayscale_system_tags_entity() {
        use crate::core::frame::{Frame, FrameMeta};
        use bevy_ecs::world::World;

        let mut world = World::new();

        let meta = FrameMeta {
            index: 0,
            timestamp_us: 0,
            source: "test".into(),
        };
        let data = Array3::from_elem((2, 2, 3), 200u8);
        world.spawn(Frame::new(meta, data));

        // Roda o sistema diretamente no world
        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(grayscale_system);
        schedule.run(&mut world);

        // Verifica tag + shape do frame convertido
        let mut q = world.query::<(&Frame, &GrayscaleTag)>();
        let (frame, _) = q.single(&world);
        assert_eq!(frame.channels(), 1);
    }
}
