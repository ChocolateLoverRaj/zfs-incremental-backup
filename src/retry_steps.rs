#[derive(Debug, Clone, Copy)]
pub enum RetryStepOutput<State, Output> {
    NotFinished(State),
    Finished(Output),
}

/// Ideally the `save_fn` writes to a file so that even if the program unexpectedly terminates (due to power-off, program being terminated by the OS, panic).
pub trait StateSaver<State, SaveError> {
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
pub trait StepDoer<State, Output, StepError, SaveError> {
    async fn do_step(
        &mut self,
        state: State,
        state_saver: &mut impl StateSaver<State, SaveError>,
    ) -> Result<RetryStepOutput<State, Output>, StepError>;
}

/// This function makes it really easy to retry an operation that involves multiple steps and continue where it left off, even after unexpected program termination.
/// This function is insanely simple but that's okay.
pub async fn retry_with_steps<State, StepError, StepDoerType, StateSaverType, SaveError, Output>(
    mut state: State,
    step_doer: &mut StepDoerType,
    save_fn: &mut StateSaverType,
) -> Result<Output, StepError>
where
    StepDoerType: StepDoer<State, Output, StepError, SaveError>,
    StateSaverType: StateSaver<State, SaveError>,
{
    Ok(loop {
        match step_doer.do_step(state, save_fn).await? {
            RetryStepOutput::NotFinished(new_state) => {
                state = new_state;
            }
            RetryStepOutput::Finished(output) => break output,
        }
    })
}
