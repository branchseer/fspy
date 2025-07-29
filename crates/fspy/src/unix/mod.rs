// use std::{ffi::OsStr, os::unix::ffi::OsStrExt as _, path::Path};

// use crate::command::Command;
// use allocator_api2::{SliceExt, alloc::Allocator, vec::Vec};
// use fspy_shared::unix::cmdinfo::CommandInfo;

// fn alloc_os_str<'a>(bump: impl Allocator + 'a, src: &OsStr) -> &'a OsStr {
//     OsStr::from_bytes(SliceExt::to_vec_in(src.as_bytes(), bump).leak())
// }

// impl Command {
//     pub fn with_info<'a, A: Allocator + Copy + 'a, E>(
//         &mut self,
//         bump: A,
//         f: impl FnOnce(&mut CommandInfo<'a, A>) -> Result<(), E>,
//     ) -> Result<(), E> {
//         let mut arg_vec = Vec::with_capacity_in(self.args.len() + 1, bump);

//         let arg0 = if let Some(arg0) = self.arg0.as_ref() {
//             arg0.as_os_str()
//         } else {
//             self.program.as_os_str()
//         };
//         arg_vec.push(alloc_os_str(bump, arg0));
//         arg_vec.extend(
//             self.args
//                 .iter()
//                 .map(|arg| alloc_os_str(bump, arg.as_os_str())),
//         );

//         let mut env_vec = Vec::with_capacity_in(self.envs.len(), bump);
//         for (name, value) in &self.envs {
//             let name = alloc_os_str(bump, &name);
//             let value = alloc_os_str(bump, &value);
//             env_vec.push((name, value));
//         }

//         let mut info = CommandInfo {
//             program: Path::new(alloc_os_str(bump, self.program.as_os_str())),
//             args: arg_vec,
//             envs: env_vec,
//         };

//         f(&mut info)?;

//         self.program = info.program.as_os_str().to_os_string();

//         let mut args = info.args.into_iter();
//         self.arg0 = Some(args.next().unwrap().to_os_string());
//         self.args = args.map(|arg| arg.to_os_string()).collect();
//         self.envs = info
//             .envs
//             .into_iter()
//             .map(|(name, value)| (name.to_os_string(), value.to_os_string()))
//             .collect();
//         Ok(())
//     }
// }
