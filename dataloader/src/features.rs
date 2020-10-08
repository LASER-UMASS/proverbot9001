/* *********************************************************************** */
//
//    This file is part of Proverbot9001.
//
//    Proverbot9001 is free software: you can redistribute it and/or modify
//    it under the terms of the GNU General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    Proverbot9001 is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU General Public License for more details.
//
//    You should have received a copy of the GNU General Public License
//    along with Proverbot9001.  If not, see <https://www.gnu.org/licenses/>.
//
//    Copyright 2019 Alex Sanchez-Stern and Yousef Alhessi
//
/* *********************************************************************** */

use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::{BinaryHeap, HashMap};
use std::fs::File;

use crate::scraped_data::*;
use crate::tokenizer::get_symbols;
use rayon::prelude::*;

pub const VEC_FEATURES_SIZE: i64 = 1;

pub fn context_features(
    args: &DataloaderArgs,
    tmap: &TokenMap,
    data: &Vec<ScrapedTactic>,
) -> (LongTensor2D, FloatTensor2D) {
    let (best_hyps, best_hyp_scores): (Vec<&str>, Vec<f64>) = data
        .par_iter()
        .map(|scraped| {
            best_scored_hyp(
                &scraped.context.focused_hyps(),
                &scraped.context.focused_goal(),
            )
        })
        .unzip();

    let word_features = data
        .iter()
        .zip(best_hyps)
        .map(|(scraped, best_hyp): (&ScrapedTactic, &str)| {
            vec![
                prev_tactic_feature(tmap, &scraped.prev_tactics),
                goal_head_feature(tmap, &scraped.context.focused_goal()),
                hyp_head_feature(tmap, best_hyp),
            ]
        })
        .collect();

    let vec_features = best_hyp_scores
        .into_iter()
        .zip(data.iter())
        .map(|(score, _datum)| {
            vec![
                score,
                // (std::cmp::min(get_symbols(&datum.context.focused_goal()).len(), 100) as f64) / 100.0,
                // (std::cmp::min(datum.context.focused_hyps().len(), 20) as f64) / 20.0
            ]
        })
        .collect();

    (word_features, vec_features)
}

pub fn sample_context_features(
    args: &DataloaderArgs,
    tmap: &TokenMap,
    _relevant_lemmas: &Vec<String>,
    prev_tactics: &Vec<String>,
    hypotheses: &Vec<String>,
    goal: &String,
) -> (LongTensor1D, FloatTensor1D) {
    let (best_hyp, best_score) = best_scored_hyp(
        &hypotheses,
        &goal,
    );
    let word_features = vec![
        prev_tactic_feature(tmap, &prev_tactics),
        goal_head_feature(tmap, &goal),
        hyp_head_feature(tmap, best_hyp),
    ];
    let vec_features = vec![
        best_score, // (std::cmp::min(get_symbols(&goal).len(), 100) as f64) / 100.0,
                   // (std::cmp::min(hypotheses.len(), 20) as f64) / 20.0
    ];
    (word_features, vec_features)
}

// index of the previous tactic, or zero if it's not
// in the index table, or there is no previous tactic.
pub fn prev_tactic_feature(tmap: &TokenMap, prev_tactics: &Vec<String>) -> i64 {
    match prev_tactics
        .last()
        .and_then(|tac| get_stem(tac))
        .and_then(|first_token| tmap.tactic_to_index.get(&first_token))
    {
        Some(idx) => (idx + 1) as i64,
        None => 0,
    }
}

pub fn goal_head_feature(tmap: &TokenMap, goal: &str) -> i64 {
    match goal.split_whitespace().next() {
        None => 0,
        Some(first_token) => match tmap.goal_token_to_index.get(first_token) {
            None => 1,
            Some(idx) => (idx + 2) as i64,
        },
    }
}

pub fn hyp_head_feature(tmap: &TokenMap, best_hyp: &str) -> i64 {
    match best_hyp
        .split_whitespace()
        .next()
        .and_then(|first_token| tmap.hyp_token_to_index.get(first_token))
    {
        Some(idx) => (idx + 1) as i64,
        None => 0,
    }
}

#[pyclass(dict, module = "dataloader")]
#[derive(Clone, Serialize, Deserialize)]
pub struct TokenMap {
    tactic_to_index: HashMap<String, usize>,
    goal_token_to_index: HashMap<String, usize>,
    hyp_token_to_index: HashMap<String, usize>,
}

pub type PickleableTokenMap = (
    HashMap<String, usize>,
    HashMap<String, usize>,
    HashMap<String, usize>,
);

impl<'source> pyo3::FromPyObject<'source> for TokenMap {
    fn extract(ob: &'source pyo3::types::PyAny) -> pyo3::PyResult<TokenMap> {
        let cls: &TokenMap = pyo3::PyTryFrom::try_from(ob)?;
        Ok(cls.clone())
    }
}

fn flip_vec<T>(vec: Vec<T>) -> HashMap<T, usize>
where
    T: std::hash::Hash + std::cmp::Eq,
{
    let mut result = HashMap::new();
    for (idx, val) in vec.into_iter().enumerate() {
        result.insert(val, idx);
    }
    result
}

impl TokenMap {
    pub fn initialize(init_data: &Vec<ScrapedTactic>, count: usize) -> TokenMap {
        let index_to_tactic = index_common(
            init_data
                .iter()
                .flat_map(|scraped| get_stem(&scraped.tactic)),
            count,
        );
        let index_to_hyp_token = index_common(
            init_data.iter().flat_map(|scraped| {
                scraped
                    .context
                    .focused_hyps()
                    .iter()
                    .map(|hyp| hyp.split_whitespace().next().unwrap().to_string())
            }),
            count,
        );
        let index_to_goal_token = index_common(
            init_data
                .iter()
                .flat_map(|scraped| scraped.context.focused_goal().split_whitespace().next())
                .map(|s| s.to_string()),
            count,
        );

        TokenMap {
            tactic_to_index: flip_vec(index_to_tactic),
            goal_token_to_index: flip_vec(index_to_goal_token),
            hyp_token_to_index: flip_vec(index_to_hyp_token),
        }
    }
    pub fn word_features_sizes(&self) -> Vec<i64> {
        // Add one to each of these to account for the UNKNOWN token
        vec![
            (self.tactic_to_index.len() + 1) as i64,
            (self.goal_token_to_index.len() + 2) as i64,
            (self.hyp_token_to_index.len() + 1) as i64,
        ]
    }

    pub fn to_dicts(&self) -> PickleableTokenMap {
        (
            self.tactic_to_index.clone(),
            self.goal_token_to_index.clone(),
            self.hyp_token_to_index.clone(),
        )
    }

    pub fn from_dicts(dicts: PickleableTokenMap) -> TokenMap {
        TokenMap {
            tactic_to_index: dicts.0,
            goal_token_to_index: dicts.1,
            hyp_token_to_index: dicts.2,
        }
    }

    pub fn save_to_text(&self, filename: &str) {
        let mut index_to_tactic = vec![""; self.tactic_to_index.len()];
        for (tactic, index) in self.tactic_to_index.iter() {
            assert!(
                index < &self.tactic_to_index.len(),
                "index is {}, but there are only {} tactics",
                index,
                self.tactic_to_index.len()
            );
            index_to_tactic[*index] = tactic;
        }
        let mut index_to_goal_token = vec![""; self.goal_token_to_index.len()];
        for (goal_token, index) in self.goal_token_to_index.iter() {
            assert!(
                index < &self.goal_token_to_index.len(),
                "index is {}, but there are only {} goal tokens",
                index,
                self.goal_token_to_index.len()
            );
            index_to_goal_token[*index] = goal_token;
        }
        let mut index_to_hyp_token = vec![""; self.hyp_token_to_index.len()];
        for (hyp_token, index) in self.hyp_token_to_index.iter() {
            assert!(
                index < &self.hyp_token_to_index.len(),
                "index is {}, but there are only {} hyp tokens",
                index,
                self.hyp_token_to_index.len()
            );
            index_to_hyp_token[*index] = hyp_token;
        }

        let mut data = HashMap::new();
        data.insert("tactics", index_to_tactic);
        data.insert("goal_tokens", index_to_goal_token);
        data.insert("hyp_tokens", index_to_hyp_token);

        let file = File::create(filename).unwrap();
        serde_json::to_writer(file, &data).unwrap();
    }

    pub fn load_from_text(filename: &str) -> TokenMap {
        let file = File::open(filename)
            .expect(&format!("Couldn't find features file at \"{}\"", filename));
        let json_data = serde_json::from_reader(file).expect("Couldn't parse json data");
        let (goal_tokens, tactics, hyp_tokens) = match json_data {
            serde_json::Value::Object(vals) => {
                match (vals["goal_tokens"].clone(),
                       vals["tactics"].clone(),
                       vals["hyp_tokens"].clone()) {
                    (
                        serde_json::Value::Array(gts),
                        serde_json::Value::Array(ts),
                        serde_json::Value::Array(hts),
                    ) => (
                        gts.iter().map(|gt| match gt {
                            serde_json::Value::String(s) => s.clone(),
                            _ => panic!("Invalid data"),
                        })
                        .collect::<Vec<_>>(),
                        ts.iter().map(|t| match t {
                            serde_json::Value::String(s) => s.clone(),
                            _ => panic!("Invalid data"),
                        })
                        .collect::<Vec<_>>(),
                        hts.iter().map(|ht| match ht {
                            serde_json::Value::String(s) => s.clone(),
                            _ => panic!("Invalid data"),
                        })
                        .collect::<Vec<_>>(),
                    ),
                    _ => panic!("Invalid data"),
                }
            }
            _ => panic!("Json data is not an object!"),
        };
        TokenMap {
            tactic_to_index: flip_vec(tactics),
            goal_token_to_index: flip_vec(goal_tokens),
            hyp_token_to_index: flip_vec(hyp_tokens),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct ScoredString<'a> {
    score: i64,
    contents: &'a str,
}

fn index_common<'a>(items: impl Iterator<Item = String>, n: usize) -> Vec<String> {
    let mut counts = HashMap::new();
    for item in items {
        *counts.entry(item).or_insert(0) += 1;
    }
    let mut heap: BinaryHeap<ScoredString> = counts
        .iter()
        .map(|(s, c)| ScoredString {
            score: *c,
            contents: s,
        })
        .collect();
    let mut result = Vec::new();
    for _ in 0..n {
        match heap.pop() {
            Some(v) => result.push(v.contents.to_owned()),
            None => break,
        }
    }
    result
}
pub fn ratcliff_obershelp_string_similarity(s1: &str, s2: &str) -> f64 {
    fn longest_common_substring_idxs(s1: &str, s2: &str) -> ((usize, usize), (usize, usize)) {
        let mut max_length = 0;
        let mut ending_index_1 = s1.len();
        let mut ending_index_2 = s2.len();
        let mut lookup = vec![vec![0; s2.len()+1]; s1.len()+1];

        for (i, c1) in s1.chars().enumerate(){
            for (j, c2) in s2.chars().enumerate() {
                if c1 == c2 {
                    lookup[i+1][j+1] = lookup[i][j] + 1;
                    if lookup[i+1][j+1] > max_length {
                        max_length = lookup[i+1][j+1];
                        ending_index_1 = i+1;
                        ending_index_2 = j+1;
                    }
                }
            }
        }
        ((ending_index_1 - max_length, ending_index_1),
         (ending_index_2 - max_length, ending_index_2))
    }
    fn matching_characters(s1: &str, s2: &str) -> usize {
        let ((l1, r1), (l2, r2)) = longest_common_substring_idxs(s1, s2);
        assert_eq!(r1 - l1, r2 - l2);
        if l1 == r1 {
            0
        } else {
            let left_rec = if l1 > 0 && l2 > 0 {
                matching_characters(&s1[..l1], &s2[..l2])
            } else { 0 };
            let right_rec = if r1 < s1.len() && r2 < s2.len() {
                matching_characters(&s1[r1..], &s2[r2..])
            } else { 0 };
            left_rec + (r1 - l1) + right_rec
        }
    }
    (2.0 * matching_characters(s1, s2) as f64) / ((s1.len() + s2.len()) as f64)
}

pub fn score_hyps<'a>(
    hyps: &Vec<String>,
    goal: &String,
) -> Vec<f64> {
    hyps.into_iter()
        .map(|hyp| {
            ratcliff_obershelp_string_similarity(get_hyp_type(hyp), goal)
        })
        .collect()
}

fn best_scored_hyp<'a>(
    hyps: &'a Vec<String>,
    goal: &String,
) -> (&'a str, f64) {
    let mut best_hyp = "";
    let mut best_score = 1.0;
    for hyp in hyps.iter() {
        let score = ratcliff_obershelp_string_similarity(get_hyp_type(hyp), goal);
        if score < best_score {
            best_score = score;
            best_hyp = &hyp;
        }
    }
    (best_hyp, best_score)
}
