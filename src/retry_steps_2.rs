use std::fmt::Debug;

use shallowclone::ShallowClone;

#[derive(Clone, Copy, Debug, ShallowClone)]
pub struct RetryStepNotFinished2<M, P> {
    // Keep data in memory (not persisted) between steps
    pub memory_data: M,
    pub persistent_data: P,
}

#[derive(Debug, Clone, Copy, ShallowClone)]
pub enum RetryStepOutput2<M, P, Output> {
    NotFinished(RetryStepNotFinished2<M, P>),
    Finished(Output),
}

/// Ideally the `save_fn` writes to a file so that even if the program unexpectedly terminates (due to power-off, program being terminated by the OS, panic).
pub trait StateSaver2<State, SaveError> {
    async fn save_state(&self, state: &State) -> Result<(), SaveError>;
}

// impl<F, Fut, T, State, SaveError> StateSaver<State, SaveError> for T
// where
//     Fut: Future<Output = Result<(), SaveError>>,
//     F: FnMut(&State) -> Fut,
//     T: AsMut<F>,
// {
//     fn save_state<'a>(
//         &'a mut self,
//         state: &'a State,
//     ) -> impl Future<Output = Result<(), SaveError>> {
//         (self.as_mut())(&state)
//     }
// }

/// The `match_fn` is responsible for saving any changes to the state it makes by calling the `save_fn`. It doesn't need to save the state after the last step because it will be saved after this function returns.
/// The `match_fn` needs to be repeat-proof in every step. For example, assume a file exists and delete it because it could've been deleted previously and the state might not have updated.
/// Be careful to avoid infinite loops!
pub trait StepDoer2<M, P, Output, StepError, SaveError> {
    async fn do_step(
        &mut self,
        memory_data: M,
        persitent_data: P,
    ) -> Result<RetryStepOutput2<M, P, Output>, StepError>;
}

/// This function makes it really easy to retry an operation that involves multiple steps and continue where it left off, even after unexpected program termination.
/// This function is insanely simple but that's okay.
pub async fn retry_with_steps_2<
    M,
    P,
    StepError,
    StepDoerType,
    StateSaverType,
    SaveError: Debug,
    Output,
>(
    initial_state: RetryStepNotFinished2<M, P>,
    step_doer: &mut StepDoerType,
    state_saver: &mut StateSaverType,
) -> Result<Output, StepError>
where
    StepDoerType: StepDoer2<M, P, Output, StepError, SaveError>,
    StateSaverType: StateSaver2<P, SaveError>,
{
    Ok({
        let mut memory_data = initial_state.memory_data;
        let mut persistent_data = initial_state.persistent_data;
        loop {
            match step_doer.do_step(memory_data, persistent_data).await? {
                RetryStepOutput2::NotFinished(new_data) => {
                    memory_data = new_data.memory_data;
                    state_saver
                        .save_state(&new_data.persistent_data)
                        .await
                        .unwrap();
                    persistent_data = new_data.persistent_data;
                }
                RetryStepOutput2::Finished(output) => break output,
            }
        }
    })
}
