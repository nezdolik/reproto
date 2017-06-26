use super::*;
use super::errors::*;

#[derive(Debug)]
pub struct MatchMember<'input> {
    pub condition: RpLoc<MatchCondition<'input>>,
    pub value: RpLoc<Value<'input>>,
}

impl<'input> IntoModel for MatchMember<'input> {
    type Output = RpMatchMember;

    fn into_model(self) -> Result<RpMatchMember> {
        let member = RpMatchMember {
            condition: self.condition.into_model()?,
            value: self.value.into_model()?,
        };

        Ok(member)
    }
}