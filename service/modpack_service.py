import logging
import os
import tempfile
import zipfile

import requests

class ModpackService:
    def __init__(self, log_file='app.log'):
        logging.basicConfig(filename=log_file, filemode='w',
                            format='%(name)s - %(levelname)s - %(message)s')
        self.logger = logging.getLogger('ModpackService')

    def download_modpack(self, modpack_url, progress_callback=None):
        self.logger.info(f"Starting download from {modpack_url}")
        temp_dir = tempfile.gettempdir()
        file_path = os.path.join(temp_dir, 'modpack.zip')

        with requests.get(modpack_url, stream=True) as response:
            if response.status_code != 200:
                self.logger.error(f"Failed to download file: {response.status_code}")
                raise Exception(f"Failed to download file: {response.status_code}")

            total_size = int(response.headers.get('Content-Length', 0))
            bytes_read = 0

            with open(file_path, 'wb') as file:
                for chunk in response.iter_content(chunk_size=1024):
                    if chunk:
                        file.write(chunk)
                        bytes_read += len(chunk)

                        if progress_callback:
                            progress_callback(bytes_read, total_size)

        self.logger.info(f"Download completed: {file_path}")
        return file_path

    def install_modpack(self, launcher_path, temp_modpack_path, progress_callback=None):
        self.logger.info(f"Starting modpack installation at {temp_modpack_path}")

        if not os.path.exists(launcher_path):
            os.makedirs(launcher_path)

        instances_path = os.path.join(launcher_path, "instances")

        if not os.path.exists(instances_path):
            os.makedirs(instances_path)

        if not temp_modpack_path.endswith(".zip"):
            raise Exception(f"File {temp_modpack_path} is not a zip file")

        with zipfile.ZipFile(temp_modpack_path, 'r') as zip_ref:
            total_files = len(zip_ref.infolist())
            extracted_files = 0

            for file in zip_ref.infolist():
                self.logger.info(f'Extracting file: {file.filename}')
                zip_ref.extract(file, instances_path)
                extracted_files += 1

                if progress_callback:
                    progress_callback(extracted_files, total_files)

        os.remove(temp_modpack_path)
        self.logger.info(f"Installation completed: {instances_path}")