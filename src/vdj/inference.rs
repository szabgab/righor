use crate::sequence::utils::difference_as_i64;
use crate::sequence::{DAlignment, VJAlignment};
use crate::shared::feature::*;
use crate::shared::utils::{insert_in_order, InferenceParameters};
use crate::vdj::{Event, Model, Sequence, StaticEvent};
use anyhow::Result;
use itertools::iproduct;
#[cfg(all(feature = "py_binds", feature = "py_o3"))]
use pyo3::{pyclass, pymethods};

#[derive(Default, Clone, Debug)]
#[cfg_attr(all(feature = "py_binds", feature = "py_o3"), pyclass(get_all))]
pub struct Features {
    pub v: CategoricalFeature1,
    pub delv: CategoricalFeature1g1,
    pub dj: CategoricalFeature2,
    pub delj: CategoricalFeature1g1,
    pub deld: CategoricalFeature2g1,
    // pub nb_insvd: CategoricalFeature1,
    // pub nb_insdj: CategoricalFeature1,
    pub insvd: InsertionFeature,
    pub insdj: InsertionFeature,
    pub error: ErrorPoisson,
}

impl Features {
    pub fn new(model: &Model, inference_params: &InferenceParameters) -> Result<Features> {
        Ok(Features {
            v: CategoricalFeature1::new(&model.p_v)?,
            delv: CategoricalFeature1g1::new(&model.p_del_v_given_v)?,
            dj: CategoricalFeature2::new(&model.p_dj)?,
            delj: CategoricalFeature1g1::new(&model.p_del_j_given_j)?,
            deld: CategoricalFeature2g1::new(&model.p_del_d3_del_d5)?, // dim: (d3, d5, d)
            // nb_insvd: CategoricalFeature1::new(&model.p_ins_vd)?,
            // nb_insdj: CategoricalFeature1::new(&model.p_ins_dj)?,
            insvd: InsertionFeature::new(
                &model.p_ins_vd,
                &model.first_nt_bias_ins_vd,
                &model.markov_coefficients_vd,
            )?,
            insdj: InsertionFeature::new(
                &model.p_ins_dj,
                &model.first_nt_bias_ins_dj,
                &model.markov_coefficients_dj,
            )?,
            error: ErrorPoisson::new(model.error_rate, inference_params.min_likelihood_error)?,
        })
    }

    // Return an iterator over V, D and J
    fn range_v<'a>(
        &self,
        sequence: &'a Sequence,
    ) -> impl Iterator<Item = (&'a VJAlignment, usize)> {
        iproduct!(sequence.v_genes.iter(), 0..self.delv.dim().0)
    }

    // Return an iterator over Events
    fn range_dj<'a>(
        &self,
        sequence: &'a Sequence,
    ) -> impl Iterator<Item = (&'a VJAlignment, usize, &'a DAlignment, usize, usize)> {
        iproduct!(
            sequence.j_genes.iter(),
            0..self.delj.dim().0,
            sequence.d_genes.iter(),
            0..self.deld.dim().1,
            0..self.deld.dim().0
        )
    }

    fn likelihood_v(&self, v: &VJAlignment, delv: usize) -> f64 {
        self.v.likelihood(v.index)
            * self.delv.likelihood((delv, v.index))
            * self.error.likelihood(v.nb_errors(delv))
    }

    fn likelihood_dj(&self, e: &Event) -> f64 {
        // Estimate the likelihood of the d/j portion of the alignment
        // First check that nothing overlaps

        // v_end: position of the last element of the V gene + 1
        let v_end = difference_as_i64(e.v.end_seq, e.delv);
        let d_start = e.d.pos + e.deld5;
        let d_end = e.d.pos + e.d.len() - e.deld3;
        let j_start = e.j.start_seq + e.delj;

        if (v_end > (d_start as i64)) | (d_start > d_end) | (d_end > j_start) {
            return 0.;
        }
        // println!("{} {} {} {} {}", e.d.pos, v_end, d_start, d_end, j_start);
        // println!("{}", self.deld.likelihood((e.deld3, e.deld5, e.d.index)));
        // println!("insdj{}", self.insdj.likelihood_length(j_start - d_end));
        // println!(
        //     "derr {} {}  {}",
        //     e.deld5,
        //     e.deld3,
        //     e.d.nb_errors(e.deld5, e.deld3)
        // );
        // println!(
        //     "insvd {}",
        //     self.insvd
        //         .likelihood_length((d_start as i64 - v_end) as usize)
        // );
        // println!();

        // Then compute the likelihood of each part (including insertion length)
        // We ignore the V/delV part (already computed)e.j.index) *
        self.dj.likelihood((e.d.index, e.j.index))
            * self.delj.likelihood((e.delj, e.j.index))
            * self.deld.likelihood((e.deld3, e.deld5, e.d.index))
            * self
                .insvd
                .likelihood_length((d_start as i64 - v_end) as usize)
            * self.insdj.likelihood_length(j_start - d_end)
            * self.error.likelihood(e.d.nb_errors(e.deld5, e.deld3))
            * self.error.likelihood(e.j.nb_errors(e.delj))
    }

    pub fn infer(
        &mut self,
        sequence: &Sequence,
        inference_params: &InferenceParameters,
        nb_best_events: usize,
    ) -> (f64, Vec<(f64, StaticEvent)>) {
        let mut probability_generation: f64 = 0.;
        let mut best_events = Vec::<(f64, StaticEvent)>::new();
        // Update all the marginals
        for (v, delv) in self.range_v(sequence) {
            let lhood_v = self.likelihood_v(v, delv);
            // drop that specific recombination event if the likelihood is too low
            if lhood_v < inference_params.min_likelihood {
                continue;
            }

            for (j, delj, d, deld5, deld3) in self.range_dj(sequence) {
                let e = Event {
                    v,
                    j,
                    d,
                    delv,
                    delj,
                    deld3,
                    deld5,
                };

                let lhood_dj = self.likelihood_dj(&e);

                let mut l_total = lhood_v * lhood_dj;
                // println!("AG {}", l_total);
                // drop that specific recombination event if the likelihood is too low
                if l_total < inference_params.min_likelihood {
                    continue;
                }

                // extract both inserted sequences
                let (insvd, insdj) = sequence.get_insertions_vd_dj(&e);

                l_total *= self.insvd.likelihood_sequence(&insvd);
                l_total *= self.insdj.likelihood_sequence(&insdj);

                // drop that specific recombination event if the likelihood is too low
                if l_total < inference_params.min_likelihood {
                    continue;
                }
                if nb_best_events > 0 {
                    if (best_events.len() < nb_best_events)
                        || (best_events.last().unwrap().0 < l_total)
                    {
                        best_events = insert_in_order(
                            best_events,
                            (l_total, e.to_static(insvd.clone(), insdj.clone())),
                        );
                        best_events.truncate(nb_best_events);
                    }
                }
                probability_generation += l_total;

                self.v.dirty_update(e.v.index, l_total);
                self.dj.dirty_update((e.d.index, e.j.index), l_total);
                self.delv.dirty_update((e.delv, e.v.index), l_total);
                self.delj.dirty_update((e.delj, e.j.index), l_total);
                self.deld
                    .dirty_update((e.deld3, e.deld5, e.d.index), l_total);
                // self.nb_insvd.dirty_update(insvd.len(), l_total);
                // self.nb_insdj.dirty_update(insdj.len(), l_total);
                self.insvd.dirty_update(&insvd, l_total);
                self.insdj.dirty_update(&insdj, l_total);
                self.error.dirty_update(
                    e.j.nb_errors(delj) + e.v.nb_errors(delv) + e.d.nb_errors(deld5, deld3),
                    l_total,
                );
            }
        }
        return (probability_generation, best_events);
    }

    pub fn cleanup(&self) -> Result<Features> {
        // Compute the new marginals for the next round
        Ok(Features {
            v: self.v.cleanup()?,
            dj: self.dj.cleanup()?,
            delv: self.delv.cleanup()?,
            delj: self.delj.cleanup()?,
            deld: self.deld.cleanup()?,
            // nb_insvd: self.nb_insvd.cleanup()?,
            // nb_insdj: self.nb_insdj.cleanup()?,
            insvd: self.insvd.cleanup()?,
            insdj: self.insdj.cleanup()?,
            error: self.error.cleanup()?,
        })
    }
}

#[cfg(not(all(feature = "py_binds", feature = "py_o3")))]
impl Features {
    pub fn average(features: Vec<Features>) -> Result<Features> {
        Ok(Features {
            v: CategoricalFeature1::average(features.iter().map(|a| a.v.clone()))?,
            delv: CategoricalFeature1g1::average(features.iter().map(|a| a.delv.clone()))?,
            dj: CategoricalFeature2::average(features.iter().map(|a| a.dj.clone()))?,
            delj: CategoricalFeature1g1::average(features.iter().map(|a| a.delj.clone()))?,
            deld: CategoricalFeature2g1::average(features.iter().map(|a| a.deld.clone()))?,
            // nb_insvd: CategoricalFeature1::average(features.iter().map(|a| a.nb_insvd.clone()))?,
            // nb_insdj: CategoricalFeature1::average(features.iter().map(|a| a.nb_insdj.clone()))?,
            insvd: InsertionFeature::average(features.iter().map(|a| a.insvd.clone()))?,
            insdj: InsertionFeature::average(features.iter().map(|a| a.insdj.clone()))?,
            error: ErrorPoisson::average(features.iter().map(|a| a.error.clone()))?,
        })
    }
}

#[cfg(all(feature = "py_binds", feature = "py_o3"))]
#[pymethods]
impl Features {
    #[staticmethod]
    pub fn average(features: Vec<Features>) -> Result<Features> {
        Ok(Features {
            v: CategoricalFeature1::average(features.iter().map(|a| a.v.clone()))?,
            delv: CategoricalFeature1g1::average(features.iter().map(|a| a.delv.clone()))?,
            dj: CategoricalFeature2::average(features.iter().map(|a| a.dj.clone()))?,
            delj: CategoricalFeature1g1::average(features.iter().map(|a| a.delj.clone()))?,
            deld: CategoricalFeature2g1::average(features.iter().map(|a| a.deld.clone()))?,
            // nb_insvd: CategoricalFeature1::average(features.iter().map(|a| a.nb_insvd.clone()))?,
            // nb_insdj: CategoricalFeature1::average(features.iter().map(|a| a.nb_insdj.clone()))?,
            insvd: InsertionFeature::average(features.iter().map(|a| a.insvd.clone()))?,
            insdj: InsertionFeature::average(features.iter().map(|a| a.insdj.clone()))?,
            error: ErrorPoisson::average(features.iter().map(|a| a.error.clone()))?,
        })
    }
}
