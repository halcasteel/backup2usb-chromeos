import { useState } from 'react';
import {
  Box,
  Flex,
  VStack,
  HStack,
  Text,
  Progress,
  Select,
  Switch,
  Button,
  List,
  ListItem,
  Badge,
  Icon,
  Divider,
  CircularProgress,
  CircularProgressLabel,
  Alert,
  AlertIcon,
  AlertTitle,
  AlertDescription,
  useColorModeValue,
} from '@chakra-ui/react';
import { FaFolder, FaCheck, FaClock } from 'react-icons/fa';
import { BackupStatus } from '../types/backup';
import { formatBytes, formatDuration } from '../utils/format';

interface BackupTabProps {
  status: BackupStatus | null;
}

export function BackupTab({ status }: BackupTabProps) {
  const [profile, setProfile] = useState('Full Backup');
  const [dryRun, setDryRun] = useState(false);
  const [sortBy, setSortBy] = useState<'name' | 'size'>('name');
  const [selectedDirs, setSelectedDirs] = useState<Set<string>>(new Set());

  const bgColor = useColorModeValue('gray.50', 'gray.800');
  const cardBg = useColorModeValue('white', 'gray.700');
  const borderColor = useColorModeValue('gray.200', 'gray.600');
  const progressBg = useColorModeValue('gray.200', 'gray.600');
  const textMuted = useColorModeValue('gray.600', 'gray.400');

  if (!status) {
    return (
      <Box p={6}>
        <Text color={textMuted}>Loading backup status...</Text>
      </Box>
    );
  }

  const currentDir = status.directories.find(d => d.status === 'in_progress');
  const completionPercentage = status.totalSize > 0 
    ? (status.completedSize / status.totalSize) * 100 
    : 0;

  const toggleSelection = (dirName: string) => {
    const newSelection = new Set(selectedDirs);
    if (newSelection.has(dirName)) {
      newSelection.delete(dirName);
    } else {
      newSelection.add(dirName);
    }
    setSelectedDirs(newSelection);
  };

  const toggleAll = () => {
    if (selectedDirs.size === status.directories.length) {
      setSelectedDirs(new Set());
    } else {
      setSelectedDirs(new Set(status.directories.map(d => d.name)));
    }
  };

  return (
    <Box>
      <Flex>
        {/* Left Column - Backup Progress */}
        <Box flex="1" p={6} borderRight="1px" borderColor={borderColor}>
          <VStack spacing={6} align="stretch">
            <Box>
              <Text fontSize="lg" fontWeight="bold" mb={4} color="brand.500">
                BACKUP PROGRESS
              </Text>
              
              {/* Mount Status Alert */}
              {status.mountStatus && !status.mountStatus.mounted && (
                <Alert status="error" mb={4} borderRadius="md">
                  <AlertIcon />
                  <Box>
                    <AlertTitle>USB Drive Not Mounted</AlertTitle>
                    <AlertDescription>
                      {status.mountStatus.message || `Please mount the USB drive at ${status.mountStatus.path} before starting backup.`}
                    </AlertDescription>
                  </Box>
                </Alert>
              )}
              
              <HStack spacing={4} mb={4}>
                <Text fontSize="sm" color={textMuted}>Profile:</Text>
                <Select 
                  size="sm" 
                  value={profile} 
                  onChange={(e) => setProfile(e.target.value)}
                  width="auto"
                  bg={cardBg}
                >
                  <option value="Full Backup">Full Backup</option>
                  <option value="Essential">Essential</option>
                  <option value="Media">Media</option>
                </Select>
                
                <HStack>
                  <Switch 
                    size="sm" 
                    isChecked={dryRun} 
                    onChange={(e) => setDryRun(e.target.checked)}
                    colorScheme="brand"
                  />
                  <Text fontSize="sm" color={textMuted}>Dry Run</Text>
                </HStack>
              </HStack>

              {/* Overall Progress */}
              <Box bg={cardBg} p={4} borderRadius="md" mb={6}>
                <Text fontSize="sm" color={textMuted} mb={2}>
                  Overall Progress: {completionPercentage.toFixed(1)}%
                </Text>
                <Progress 
                  value={completionPercentage} 
                  size="sm" 
                  colorScheme="brand"
                  bg={progressBg}
                  borderRadius="full"
                />
              </Box>

              {/* Circular Progress */}
              <Flex justify="center" mb={6}>
                <CircularProgress 
                  value={completionPercentage} 
                  size="200px"
                  thickness="4px"
                  color="brand.500"
                  trackColor={progressBg}
                >
                  <CircularProgressLabel>
                    <VStack spacing={0}>
                      <Text fontSize="3xl" fontWeight="bold">
                        {completionPercentage.toFixed(1)}%
                      </Text>
                      <Text fontSize="sm" color={textMuted}>Complete</Text>
                    </VStack>
                  </CircularProgressLabel>
                </CircularProgress>
              </Flex>

              {/* Action Buttons */}
              <HStack spacing={3} justify="center">
                <Button size="sm" variant="outline" onClick={toggleAll}>
                  {selectedDirs.size === status.directories.length ? 'Select None' : 'Select All'}
                </Button>
                <Button size="sm" variant="outline" onClick={() => setSortBy(sortBy === 'name' ? 'size' : 'name')}>
                  Sort by {sortBy === 'name' ? 'Size' : 'Name'}
                </Button>
              </HStack>
            </Box>

            {/* Directory List */}
            <Box>
              <List spacing={2} maxH="400px" overflowY="auto">
                {status.directories
                  .sort((a, b) => {
                    if (sortBy === 'name') {
                      return a.name.localeCompare(b.name);
                    }
                    return b.size - a.size;
                  })
                  .map((dir) => (
                    <ListItem 
                      key={dir.name}
                      p={3}
                      bg={cardBg}
                      borderRadius="md"
                      cursor="pointer"
                      onClick={() => toggleSelection(dir.name)}
                      borderWidth="1px"
                      borderColor={selectedDirs.has(dir.name) ? 'brand.500' : 'transparent'}
                      _hover={{ borderColor: 'brand.400' }}
                    >
                      <HStack spacing={3}>
                        <Box 
                          w="4" 
                          h="4" 
                          borderWidth="2px"
                          borderColor={selectedDirs.has(dir.name) ? 'brand.500' : 'gray.400'}
                          borderRadius="sm"
                          bg={selectedDirs.has(dir.name) ? 'brand.500' : 'transparent'}
                          display="flex"
                          alignItems="center"
                          justifyContent="center"
                        >
                          {selectedDirs.has(dir.name) && (
                            <Icon as={FaCheck} color="white" boxSize={2} />
                          )}
                        </Box>
                        
                        <Icon 
                          as={FaFolder} 
                          color={dir.status === 'completed' ? 'green.500' : 
                                dir.status === 'active' ? 'brand.500' : 
                                'gray.400'}
                        />
                        
                        <VStack align="start" spacing={0} flex="1">
                          <Text fontSize="sm" fontWeight="medium">
                            {dir.name}
                          </Text>
                          <Text fontSize="xs" color={textMuted}>
                            {formatBytes(dir.size)} • {dir.filesProcessed || 0} files
                          </Text>
                        </VStack>
                        
                        {dir.status === 'completed' && (
                          <Badge colorScheme="green" size="sm">
                            <Icon as={FaCheck} />
                          </Badge>
                        )}
                        {dir.status === 'active' && (
                          <Badge colorScheme="brand" size="sm">
                            {dir.progress}%
                          </Badge>
                        )}
                      </HStack>
                    </ListItem>
                  ))}
              </List>
            </Box>
          </VStack>
        </Box>

        {/* Right Column - Current Operation */}
        <Box flex="1" p={6}>
          <VStack spacing={6} align="stretch">
            <Box>
              <Text fontSize="lg" fontWeight="bold" mb={4} color="brand.500">
                CURRENT OPERATION
              </Text>
              
              {currentDir ? (
                <Box bg={cardBg} p={4} borderRadius="md">
                  <Text fontSize="lg" fontWeight="medium" mb={2}>
                    {currentDir.name}
                  </Text>
                  
                  <Divider my={3} />
                  
                  <VStack align="stretch" spacing={3}>
                    <HStack justify="space-between">
                      <Text fontSize="sm" color={textMuted}>Files Processed</Text>
                      <HStack>
                        <Text fontSize="xl" fontWeight="bold" color="brand.500">
                          {currentDir.filesProcessed || 0}
                        </Text>
                        <Text fontSize="sm" color={textMuted}>Files</Text>
                      </HStack>
                    </HStack>
                    
                    <HStack justify="space-between">
                      <Text fontSize="sm" color={textMuted}>Size Copied</Text>
                      <HStack>
                        <Text fontSize="xl" fontWeight="bold" color="brand.500">
                          {formatBytes(currentDir.bytesProcessed || 0)}
                        </Text>
                        <Text fontSize="sm" color={textMuted}>B</Text>
                      </HStack>
                    </HStack>
                    
                    <HStack justify="space-between">
                      <Text fontSize="sm" color={textMuted}>Time Elapsed</Text>
                      <HStack>
                        <Text fontSize="xl" fontWeight="bold" color="brand.500">
                          {formatDuration(Date.now() - (status.startTime || Date.now()))}
                        </Text>
                      </HStack>
                    </HStack>
                    
                    <HStack justify="space-between">
                      <Text fontSize="sm" color={textMuted}>Current Speed</Text>
                      <HStack>
                        <Text fontSize="xl" fontWeight="bold" color="brand.500">
                          {((currentDir.bytesProcessed || 0) / 
                            ((Date.now() - (status.startTime || Date.now())) / 1000) / 
                            1048576).toFixed(2)}
                        </Text>
                        <Text fontSize="sm" color={textMuted}>MB/s</Text>
                      </HStack>
                    </HStack>
                  </VStack>
                  
                  <Box mt={4}>
                    <Text fontSize="sm" color={textMuted} mb={2}>
                      Progress: {currentDir.progress}%
                    </Text>
                    <Progress 
                      value={currentDir.progress} 
                      size="sm" 
                      colorScheme="brand"
                      bg={progressBg}
                      borderRadius="full"
                    />
                  </Box>
                  
                  {currentDir.currentFile && (
                    <Box mt={4} p={3} bg={bgColor} borderRadius="md">
                      <Text fontSize="xs" color={textMuted} mb={1}>Current File:</Text>
                      <Text fontSize="sm" fontFamily="mono" noOfLines={2}>
                        {currentDir.currentFile}
                      </Text>
                    </Box>
                  )}
                </Box>
              ) : (
                <Box bg={cardBg} p={8} borderRadius="md" textAlign="center">
                  <Icon as={FaClock} boxSize={12} color={textMuted} mb={4} />
                  <Text color={textMuted}>No active backup operation</Text>
                </Box>
              )}
            </Box>
            
            {status.lastCompletedDir && (
              <Box>
                <Text fontSize="sm" fontWeight="bold" mb={2} color="brand.500">
                  LAST COMPLETED
                </Text>
                <Box bg={cardBg} p={3} borderRadius="md">
                  <HStack>
                    <Icon as={FaCheck} color="green.500" />
                    <VStack align="start" spacing={0} flex="1">
                      <Text fontSize="sm" fontWeight="medium">
                        {status.lastCompletedDir}
                      </Text>
                      <Text fontSize="xs" color={textMuted}>
                        Completed at {new Date().toLocaleTimeString()}
                      </Text>
                    </VStack>
                  </HStack>
                </Box>
              </Box>
            )}
            
            {status.nextDir && (
              <Box>
                <Text fontSize="sm" fontWeight="bold" mb={2} color="brand.500">
                  UP NEXT
                </Text>
                <Box bg={cardBg} p={3} borderRadius="md">
                  <HStack>
                    <Icon as={FaClock} color="brand.500" />
                    <Text fontSize="sm" fontWeight="medium">
                      {status.nextDir}
                    </Text>
                  </HStack>
                </Box>
              </Box>
            )}
          </VStack>
        </Box>
      </Flex>
    </Box>
  );
}