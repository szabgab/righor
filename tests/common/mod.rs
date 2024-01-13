use ihor::vdj;
use ndarray::array;

pub fn simple_model_vdj() -> vdj::Model {
    let gv1 = ihor::Gene {
        name: "V1".to_string(),
        seq: ihor::Dna::from_string("TGCTCATGCAAAAAAAAA").unwrap(),
        seq_with_pal: None,
        functional: "(F)".to_string(),
        cdr3_pos: Some(6),
    };
    let gj1 = ihor::Gene {
        name: "J1".to_string(),
        seq: ihor::Dna::from_string("GGGGGGCAGTCAGT").unwrap(),
        seq_with_pal: None,
        functional: "(F)".to_string(),
        cdr3_pos: Some(11),
    };
    let gd1 = ihor::Gene {
        name: "D1".to_string(),
        seq: ihor::Dna::from_string("TTTTTCGCTTTT").unwrap(),
        seq_with_pal: None,
        functional: "(F)".to_string(),
        cdr3_pos: None,
    };

    let mut model = ihor::vdj::Model {
        seg_vs: vec![gv1],
        seg_js: vec![gj1],
        seg_ds: vec![gd1],
        p_v: array![1.],
        p_j_given_v: array![[1.]],
        p_d_given_vj: array![[[1.]]],
        p_ins_vd: array![0.4, 0.2, 0.1, 0.1, 0.1, 0.05, 0.05],
        p_ins_dj: array![0.4, 0.2, 0.1, 0.1, 0.1, 0.05, 0.05],
        p_del_v_given_v: array![[0.1], [0.2], [0.4], [0.1], [0.05], [0.1], [0.05],],
        p_del_j_given_j: array![[0.1], [0.2], [0.4], [0.1], [0.05], [0.1], [0.05],],
        p_del_d3_del_d5: array![
            [[0.05], [0.1], [0.05], [0.3], [0.02]],
            [[0.05], [0.1], [0.05], [0.3], [0.02]],
            [[0.05], [0.35], [0.05], [0.3], [0.02]],
        ],
        markov_coefficients_vd: array![
            [0.25, 0.25, 0.25, 0.25],
            [0.25, 0.25, 0.25, 0.25],
            [0.25, 0.25, 0.25, 0.25],
            [0.25, 0.25, 0.25, 0.25]
        ],
        markov_coefficients_dj: array![
            [0.25, 0.25, 0.25, 0.25],
            [0.25, 0.25, 0.25, 0.25],
            [0.25, 0.25, 0.25, 0.25],
            [0.25, 0.25, 0.25, 0.25]
        ],
        first_nt_bias_ins_vd: array![0.25, 0.25, 0.25, 0.25],
        first_nt_bias_ins_dj: array![0.25, 0.25, 0.25, 0.25],
        range_del_v: (-2, 4),
        range_del_j: (-2, 4),
        range_del_d3: (-1, 3),
        range_del_d5: (-1, 1),
        error_rate: 0.,
        ..Default::default()
    };

    model.initialize().unwrap();
    model
}

pub fn inference_parameters_default() -> ihor::InferenceParameters {
    ihor::InferenceParameters {
        min_log_likelihood: -600.0,
        nb_best_events: 10,
        evaluate: true,
    }
}

pub fn alignment_parameters_default() -> ihor::AlignmentParameters {
    ihor::AlignmentParameters {
        min_score_v: 0,
        min_score_j: 0,
        max_error_d: 50,
    }
}
