use crate::{BlockDevice, Controller, Directory, Error, File, Mode, TimeSource, Volume, VolumeIdx};
use core::cell::{RefCell, RefMut};

pub struct ControllerHandle<D, T>
where
    D: BlockDevice,
    T: TimeSource,
    <D as BlockDevice>::Error: core::fmt::Debug,
{
    controller: RefCell<Controller<D, T>>,
}
impl<D, T> ControllerHandle<D, T>
where
    D: BlockDevice,
    T: TimeSource,
    <D as BlockDevice>::Error: core::fmt::Debug,
{
    pub fn new(controller: Controller<D, T>) -> Self {
        Self {
            controller: RefCell::new(controller),
        }
    }
    fn controller(&self) -> Result<RefMut<Controller<D, T>>, Error<D::Error>> {
        self.controller
            .try_borrow_mut()
            .map_err(|_| Error::ControllerInUse)
    }
}
pub trait ControllerTrait: Sized {
    type Error: core::fmt::Debug;
    fn volume<'a>(&'a self, index: usize) -> Result<VolumeHandle<'a, Self>, Error<Self::Error>>;
    fn root<'a>(
        &'a self,
        volume: &'a VolumeHandle<'a, Self>,
    ) -> Result<DirectoryHandle<'a, Self>, Error<Self::Error>>;
    fn close_directory(
        &self,
        volume: &VolumeHandle<Self>,
        directory: Directory,
    ) -> Result<(), Error<Self::Error>>;
    fn file<'a>(
        &'a self,
        volume: &'a VolumeHandle<'a, Self>,
        directory: &Directory,
        name: &str,
        mode: Mode,
    ) -> Result<FileHandle<'a, Self>, Error<Self::Error>>;
    fn close_file(&self, volume: &VolumeHandle<Self>, file: File)
        -> Result<(), Error<Self::Error>>;
    fn read(
        &self,
        volume: &VolumeHandle<Self>,
        file: &mut File,
        buffer: &mut [u8],
    ) -> Result<usize, Error<Self::Error>>;
    fn write(
        &self,
        volume: &VolumeHandle<Self>,
        file: &mut File,
        data: &[u8],
    ) -> Result<usize, Error<Self::Error>>;

    fn write_root_file(
        &self,
        volume: usize,
        name: &str,
        mode: Mode,
        data: &[u8],
    ) -> Result<usize, Error<Self::Error>> {
        let volume = self.volume(volume)?;
        let root = volume.root()?;
        let mut file = root.file(name, mode)?;
        file.write(data)
    }
}
impl<D, T> ControllerTrait for ControllerHandle<D, T>
where
    D: BlockDevice,
    T: TimeSource,
    <D as BlockDevice>::Error: core::fmt::Debug,
{
    type Error = D::Error;
    fn volume(&self, index: usize) -> Result<VolumeHandle<Self>, Error<Self::Error>> {
        let mut controller = self.controller()?;
        let volume = controller.get_volume(VolumeIdx(index))?;
        Ok(VolumeHandle {
            controller: &self,
            volume: RefCell::new(volume),
        })
    }
    fn root<'a>(
        &'a self,
        volume: &'a VolumeHandle<'a, Self>,
    ) -> Result<DirectoryHandle<'a, Self>, Error<Self::Error>> {
        let mut controller = self.controller()?;
        let volume_ref = volume.volume()?;
        let directory = controller.open_root_dir(&volume_ref)?;
        Ok(DirectoryHandle {
            controller: &self,
            volume: &volume,
            directory: Some(directory),
        })
    }
    fn close_directory(
        &self,
        volume: &VolumeHandle<Self>,
        directory: Directory,
    ) -> Result<(), Error<Self::Error>> {
        let mut controller = self.controller()?;
        let volume = volume.volume()?;
        controller.close_dir(&volume, directory);
        Ok(())
    }
    fn file<'a>(
        &'a self,
        volume: &'a VolumeHandle<'a, Self>,
        directory: &Directory,
        name: &str,
        mode: Mode,
    ) -> Result<FileHandle<'a, Self>, Error<Self::Error>> {
        let mut controller = self.controller()?;
        let mut volume_ref = volume.volume()?;
        let file = controller.open_file_in_dir(&mut volume_ref, directory, name, mode)?;
        Ok(FileHandle {
            controller: &self,
            volume: &volume,
            file: Some(file),
        })
    }
    fn close_file(
        &self,
        volume: &VolumeHandle<Self>,
        file: File,
    ) -> Result<(), Error<Self::Error>> {
        let mut controller = self.controller()?;
        let volume = volume.volume()?;
        controller.close_file(&volume, file)
    }
    fn read(
        &self,
        volume: &VolumeHandle<Self>,
        file: &mut File,
        buffer: &mut [u8],
    ) -> Result<usize, Error<Self::Error>> {
        let mut controller = self.controller()?;
        let mut volume = volume.volume()?;
        controller.read(&mut volume, file, buffer)
    }
    fn write(
        &self,
        volume: &VolumeHandle<Self>,
        file: &mut File,
        data: &[u8],
    ) -> Result<usize, Error<Self::Error>> {
        let mut controller = self.controller()?;
        let mut volume = volume.volume()?;
        controller.write(&mut volume, file, data)
    }
}

pub struct VolumeHandle<'a, C>
where
    C: ControllerTrait,
    <C as ControllerTrait>::Error: core::fmt::Debug,
{
    controller: &'a C,
    volume: RefCell<Volume>,
}
impl<'a, C> VolumeHandle<'a, C>
where
    C: ControllerTrait,
    <C as ControllerTrait>::Error: core::fmt::Debug,
{
    fn volume(&self) -> Result<RefMut<Volume>, Error<C::Error>> {
        self.volume.try_borrow_mut().map_err(|_| Error::VolumeInUse)
    }

    pub fn root(&self) -> Result<DirectoryHandle<C>, Error<C::Error>> {
        self.controller.root(&self)
    }
    pub fn num_blocks(&self) -> Result<u32, Error<C::Error>> {
        let volume = self.volume()?;
        let num_blocks = match &volume.volume_type {
            crate::VolumeType::Fat(fat) => fat.num_blocks,
        };
        Ok(num_blocks.0)
    }
    pub fn blocks_per_cluster(&self) -> Result<u8, Error<C::Error>> {
        let volume = self.volume()?;
        let blocks_per_cluster = match &volume.volume_type {
            crate::VolumeType::Fat(fat) => fat.blocks_per_cluster,
        };
        Ok(blocks_per_cluster)
    }
    pub fn cluster_count(&self) -> Result<u32, Error<C::Error>> {
        let volume = self.volume()?;
        let cluster_count = match &volume.volume_type {
            crate::VolumeType::Fat(fat) => fat.cluster_count,
        };
        Ok(cluster_count)
    }
    pub fn free_clusters_count(&self) -> Result<u32, Error<C::Error>> {
        let volume = self.volume()?;
        let free_clusters_count = match &volume.volume_type {
            crate::VolumeType::Fat(fat) => fat.free_clusters_count,
        };
        Ok(free_clusters_count.unwrap_or(0))
    }
}

pub struct DirectoryHandle<'a, C>
where
    C: ControllerTrait,
    <C as ControllerTrait>::Error: core::fmt::Debug,
{
    controller: &'a C,
    volume: &'a VolumeHandle<'a, C>,
    directory: Option<Directory>,
}
impl<'a, C> DirectoryHandle<'a, C>
where
    C: ControllerTrait,
    <C as ControllerTrait>::Error: core::fmt::Debug,
{
    pub fn file(&self, name: &str, mode: Mode) -> Result<FileHandle<C>, Error<C::Error>> {
        self.controller
            .file(self.volume, &self.directory.as_ref().unwrap(), name, mode)
    }
}
impl<'a, C> Drop for DirectoryHandle<'a, C>
where
    C: ControllerTrait,
    <C as ControllerTrait>::Error: core::fmt::Debug,
{
    fn drop(&mut self) {
        if let Err(e) = self
            .controller
            .close_directory(self.volume, self.directory.take().unwrap())
        {
            log::info!("Error dropping FileHandle: {:?}", e);
        }
    }
}

pub struct FileHandle<'a, C>
where
    C: ControllerTrait,
    <C as ControllerTrait>::Error: core::fmt::Debug,
{
    controller: &'a C,
    volume: &'a VolumeHandle<'a, C>,
    file: Option<File>,
}
impl<'a, C> FileHandle<'a, C>
where
    C: ControllerTrait,
    <C as ControllerTrait>::Error: core::fmt::Debug,
{
    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error<C::Error>> {
        self.controller
            .read(self.volume, &mut self.file.as_mut().unwrap(), buffer)
    }
    pub fn write(&mut self, data: &[u8]) -> Result<usize, Error<C::Error>> {
        self.controller
            .write(self.volume, &mut self.file.as_mut().unwrap(), data)
    }
    pub fn size(&self) -> u32 {
        self.file.as_ref().unwrap().length
    }
}
impl<'a, C> Drop for FileHandle<'a, C>
where
    C: ControllerTrait,
    <C as ControllerTrait>::Error: core::fmt::Debug,
{
    fn drop(&mut self) {
        if let Err(e) = self
            .controller
            .close_file(self.volume, self.file.take().unwrap())
        {
            log::info!("Error dropping FileHandle: {:?}", e);
        }
    }
}
